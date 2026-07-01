//! The single source of truth. `AppState` is cloneable (all `Arc`) and shared
//! between Tauri commands, the axum overlay server, and the coalesced
//! broadcaster. Mutations mark a dirty flag + persist; the broadcaster turns
//! dirtiness into one `Snapshot` per tick, fanned to overlay (WS) and dashboard.

use crate::db;
use crate::domain::{
    battle::Battle,
    snapshot::{battle_view, ConnectionState, KickView, SavedBattle, Settings, Snapshot},
    song::{MediaMetadata, Song, Source},
    submit::{self, SubmitLedger},
    vote::VoteChoice,
};
use crate::error::{AppError, AppResult};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Live in-memory state behind the lock.
#[derive(Default)]
pub struct App {
    pub battle: Option<Battle>,
    pub kick: KickConn,
    pub settings: Settings,
    /// RAM-only anti-flood ledger for chat `!submit` (cleared between sessions).
    pub submit_ledger: SubmitLedger,
}

pub struct KickConn {
    pub state: ConnectionState,
    pub channel: Option<String>,
}

impl Default for KickConn {
    fn default() -> Self {
        Self {
            state: ConnectionState::Disconnected,
            channel: None,
        }
    }
}

/// The (verifier, state) pair for an in-flight official-Kick OAuth login.
/// RAM-only — a stale login after an app restart just needs a retry.
// ponytail: no TTL. The slot is single-use, guarded by a 128-bit unguessable
// `state`, reachable only over loopback, overwritten by the next login, and
// cleared on success — an abandoned entry carries no real risk. Add a timestamp
// + expiry here only if that trust boundary ever widens (e.g. non-loopback).
struct PendingOauth {
    verifier: String,
    state: String,
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<App>>,
    tx: broadcast::Sender<String>,
    db: Arc<Mutex<rusqlite::Connection>>,
    seq: Arc<AtomicU64>,
    dirty: Arc<AtomicBool>,
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
    kick_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    pending_oauth: Arc<Mutex<Option<PendingOauth>>>,
    /// Recently-seen webhook message ids, to drop Kick's redeliveries (K2).
    webhook_ids: Arc<Mutex<VecDeque<String>>>,
}

impl AppState {
    pub fn new(conn: rusqlite::Connection) -> Self {
        let (tx, _rx) = broadcast::channel::<String>(64);
        let settings = db::get_settings(&conn).unwrap_or_default();
        Self {
            inner: Arc::new(RwLock::new(App {
                settings,
                ..App::default()
            })),
            tx,
            db: Arc::new(Mutex::new(conn)),
            seq: Arc::new(AtomicU64::new(0)),
            dirty: Arc::new(AtomicBool::new(false)),
            app_handle: Arc::new(Mutex::new(None)),
            kick_tasks: Arc::new(Mutex::new(Vec::new())),
            pending_oauth: Arc::new(Mutex::new(None)),
            webhook_ids: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Record a webhook message id; returns `true` if it's new (should be
    /// processed) or `false` if it's a redelivery we've already handled.
    // ponytail: bounded FIFO of the last 512 ids, linear scan — trivial at chat
    // webhook rates; swap for a HashSet+queue only if that ever gets hot.
    pub fn webhook_id_is_new(&self, id: &str) -> bool {
        let mut q = self.webhook_ids.lock().unwrap();
        if q.iter().any(|x| x == id) {
            return false;
        }
        if q.len() >= 512 {
            q.pop_front();
        }
        q.push_back(id.to_owned());
        true
    }

    /// Test-only peek: has this webhook id been recorded (i.e. processed past the
    /// subscription gate)? Used to assert the gate without a full battle.
    #[cfg(test)]
    pub(crate) fn webhook_id_seen(&self, id: &str) -> bool {
        self.webhook_ids.lock().unwrap().iter().any(|x| x == id)
    }

    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
    }

    /// Emit a Tauri event to the frontend, if the app handle is set yet (the
    /// overlay server can start before `.setup()` finishes wiring it up).
    /// Mirrors `broadcast()`'s best-effort emit — never fails the caller.
    pub fn emit_event(&self, event: &str) {
        let handle = self.app_handle.lock().unwrap().clone();
        if let Some(h) = handle {
            let _ = h.emit(event, ());
        }
    }

    pub fn set_battle(&self, battle: Battle) {
        self.inner.write().unwrap().battle = Some(battle);
    }

    // ── mutation helpers (no lock guard ever escapes / crosses an await) ──────
    pub fn with_battle<R>(&self, f: impl FnOnce(&mut Battle) -> R) -> AppResult<R> {
        let mut app = self.inner.write().unwrap();
        let b = app
            .battle
            .as_mut()
            .ok_or_else(|| AppError::NotFound("no active battle".into()))?;
        Ok(f(b))
    }

    pub fn cast_vote(&self, user_id: String, choice: VoteChoice, now_ms: u64) -> bool {
        let mut app = self.inner.write().unwrap();
        app.battle
            .as_mut()
            .is_some_and(|b| b.cast_vote(user_id, choice, now_ms))
    }

    /// One logical second for the active countdown. `(redraw, resolved)`.
    pub fn tick_battle(&self) -> (bool, bool) {
        let mut app = self.inner.write().unwrap();
        app.battle.as_mut().map_or((false, false), Battle::tick)
    }

    pub fn set_kick(&self, state: ConnectionState, channel: Option<String>) {
        let mut app = self.inner.write().unwrap();
        app.kick = KickConn { state, channel };
    }

    pub fn set_kick_state(&self, state: ConnectionState) {
        self.inner.write().unwrap().kick.state = state;
    }

    // ── official Kick OAuth (K1) ────────────────────────────────────────────
    /// Stash the PKCE verifier + CSRF state for the in-flight login.
    pub fn start_oauth(&self, verifier: String, state: String) {
        *self.pending_oauth.lock().unwrap() = Some(PendingOauth { verifier, state });
    }

    /// Validate the callback's `state` against the pending login. Consumes the
    /// pending entry only on a match (single-use — closes the replay window); a
    /// wrong `state` leaves the real login in place so it can still complete.
    /// `None` means no login is in flight, or the state didn't match (CSRF check).
    pub fn take_oauth(&self, state: &str) -> Option<String> {
        let mut guard = self.pending_oauth.lock().unwrap();
        if guard.as_ref().is_some_and(|p| p.state == state) {
            return guard.take().map(|p| p.verifier);
        }
        None
    }

    pub async fn get_kick_auth(&self) -> AppResult<db::KickAuth> {
        self.read_db(db::get_kick_auth).await
    }

    pub async fn set_kick_creds(&self, client_id: String, client_secret: String) -> AppResult<()> {
        self.write_db(move |conn| db::set_kick_creds(conn, &client_id, &client_secret))
            .await
    }

    pub async fn set_kick_tokens(
        &self,
        access_token: String,
        refresh_token: Option<String>,
        expires_at: i64,
    ) -> AppResult<()> {
        self.write_db(move |conn| {
            db::set_kick_tokens(conn, &access_token, refresh_token.as_deref(), expires_at)
        })
        .await
    }

    /// Persist the active webhook subscription id (cleared on disconnect).
    pub async fn set_kick_subscription(&self, subscription_id: Option<String>) -> AppResult<()> {
        self.write_db(move |conn| db::set_kick_subscription(conn, subscription_id.as_deref()))
            .await
    }

    /// Local logout — K2 also deletes the remote webhook subscription before
    /// calling this.
    pub async fn clear_kick_auth(&self) -> AppResult<()> {
        self.write_db(db::clear_kick_auth).await
    }

    pub fn export_json(&self) -> AppResult<String> {
        let app = self.inner.read().unwrap();
        Ok(serde_json::to_string_pretty(&app.battle)?)
    }

    pub fn import_json(&self, json: &str) -> AppResult<()> {
        let mut battle: Battle = serde_json::from_str(json)?;
        battle.rewire(); // recompute routing + fill flags (skipped in serde)
        battle.normalize(); // untrusted boundary: clamp out-of-range `current`
        self.set_battle(battle);
        Ok(())
    }

    // ── settings ─────────────────────────────────────────────────────────────
    pub fn settings(&self) -> Settings {
        self.inner.read().unwrap().settings
    }

    pub fn default_timer_sec(&self) -> u32 {
        self.inner.read().unwrap().settings.default_timer_sec
    }

    pub async fn set_anonymous(&self, anonymous: bool) -> AppResult<()> {
        self.write_db(move |conn| db::set_anonymous(conn, anonymous))
            .await?;
        self.inner.write().unwrap().settings.anonymous = anonymous;
        self.mark_dirty();
        Ok(())
    }

    pub async fn set_default_timer(&self, sec: u32) -> AppResult<()> {
        self.write_db(move |conn| db::set_default_timer(conn, sec))
            .await?;
        self.inner.write().unwrap().settings.default_timer_sec = sec;
        Ok(())
    }

    pub async fn set_chat_submissions(&self, enabled: bool) -> AppResult<()> {
        self.write_db(move |conn| db::set_chat_submissions(conn, enabled))
            .await?;
        self.inner.write().unwrap().settings.chat_submissions = enabled;
        Ok(())
    }

    // ── chat song submissions ────────────────────────────────────────────────
    /// Synchronous anti-flood gate (runs under the lock to close the TOCTOU).
    /// `Some((source, url))` → spawn the oEmbed fetch; the ledger is already bumped.
    pub fn gate_submission(
        &self,
        user_id: &str,
        raw_url: &str,
        now_ms: u64,
    ) -> Option<(Source, String)> {
        let mut guard = self.inner.write().unwrap();
        let App {
            settings,
            battle,
            submit_ledger,
            ..
        } = &mut *guard;
        submit::evaluate(
            settings.chat_submissions,
            battle.as_ref(),
            submit_ledger,
            user_id,
            raw_url,
            now_ms,
        )
    }

    /// Append a resolved submission to the lobby, then persist. Called off the chat
    /// loop after the fetch succeeds. Re-asserts BOTH invariants under the lock —
    /// still a lobby (no bracket) AND under the cap — because the gate's checks were
    /// synchronous but the oEmbed fetch raced; without the cap re-check a distinct-
    /// userId burst overshoots GLOBAL_SONG_CAP (and triggers O(n^2) DB rewrites).
    pub async fn add_submitted_song(&self, meta: MediaMetadata, submitter: String) {
        let added = {
            let mut app = self.inner.write().unwrap();
            match app.battle.as_mut() {
                Some(b) if submit::lobby_open(b) => {
                    b.add_song(Song {
                        id: Uuid::new_v4().to_string(),
                        title: meta.title,
                        artist: meta.artist,
                        thumbnail: meta.thumbnail,
                        duration_sec: meta.duration_sec,
                        source: meta.source,
                        source_url: meta.source_url,
                        // Clamp the (untrusted) chat name before it's stored/broadcast.
                        submitter: Some(submitter.chars().take(40).collect()),
                        metadata: None,
                    });
                    true
                }
                _ => false, // bracket started / cap reached in the meantime → drop
            }
        };
        if added {
            self.persist().await;
        }
    }

    pub fn clear_submit_ledger(&self) {
        self.inner.write().unwrap().submit_ledger.clear();
    }

    // ── saved tournaments ────────────────────────────────────────────────────
    pub async fn list_battles(&self) -> AppResult<Vec<SavedBattle>> {
        self.read_db(db::list_battles).await
    }

    /// Load a saved battle and make it active. Returns false if no such id.
    pub async fn load_saved_battle(&self, id: String) -> AppResult<bool> {
        let battle = self.read_db(move |conn| db::load_battle(conn, &id)).await?;
        match battle {
            Some(b) => {
                self.set_battle(b);
                self.mark_dirty();
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub async fn delete_battle(&self, id: String) -> AppResult<()> {
        let was_active = self
            .inner
            .read()
            .unwrap()
            .battle
            .as_ref()
            .is_some_and(|b| b.id == id);
        let del_id = id.clone();
        self.write_db(move |conn| db::delete_battle(conn, &del_id))
            .await?;
        if was_active {
            // Replace the now-deleted active battle with the next latest (or none).
            let latest = self.read_db(db::load_latest).await?;
            self.inner.write().unwrap().battle = latest;
        }
        self.mark_dirty();
        Ok(())
    }

    /// Run a blocking SQLite read off the async runtime.
    async fn read_db<T, F>(&self, f: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&rusqlite::Connection) -> AppResult<T> + Send + 'static,
    {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || f(&db.lock().unwrap()))
            .await
            .map_err(|e| AppError::Other(format!("db task: {e}")))?
    }

    /// Run a blocking SQLite write off the async runtime.
    async fn write_db<F>(&self, f: F) -> AppResult<()>
    where
        F: FnOnce(&rusqlite::Connection) -> AppResult<()> + Send + 'static,
    {
        self.read_db(f).await
    }

    // ── dirty flag + broadcast ───────────────────────────────────────────────
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::SeqCst)
    }

    fn build_snapshot(&self, seq: u64) -> Snapshot {
        let app = self.inner.read().unwrap();
        Snapshot {
            seq,
            battle: app
                .battle
                .as_ref()
                .map(|b| battle_view(b, app.settings.anonymous)),
            kick: KickView {
                state: app.kick.state,
                channel: app.kick.channel.clone(),
            },
            anonymous: app.settings.anonymous,
        }
    }

    /// The current snapshot without bumping `seq` (for `get_snapshot` / WS hello).
    pub fn current_snapshot(&self) -> Snapshot {
        self.build_snapshot(self.seq.load(Ordering::SeqCst))
    }

    pub fn current_snapshot_json(&self) -> String {
        serde_json::to_string(&self.current_snapshot()).unwrap_or_else(|_| "{}".into())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Bump `seq`, build one snapshot, fan it to overlay (WS) + dashboard (event).
    /// The ONLY place `seq` advances.
    pub fn broadcast(&self) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let snap = self.build_snapshot(seq);
        if let Ok(json) = serde_json::to_string(&snap) {
            let _ = self.tx.send(json);
        }
        // Clone the handle out and drop the std Mutex guard BEFORE emitting, so
        // we never hold a sync lock across the (async-runtime) emit call.
        let handle = self.app_handle.lock().unwrap().clone();
        if let Some(h) = handle {
            let _ = h.emit("snapshot", &snap);
        }
    }

    // ── persistence ──────────────────────────────────────────────────────────
    /// Mark dirty and write the current battle to SQLite off the async runtime.
    pub async fn persist(&self) {
        self.mark_dirty();
        let battle = self.inner.read().unwrap().battle.clone();
        let Some(battle) = battle else { return };
        let db = self.db.clone();
        let res = tokio::task::spawn_blocking(move || {
            let conn = db.lock().unwrap();
            db::save_battle(&conn, &battle)
        })
        .await;
        if let Ok(Err(e)) = res {
            tracing::error!("persist failed: {e}");
        }
    }

    // ── provider task lifecycle ──────────────────────────────────────────────
    pub fn set_kick_tasks(&self, tasks: Vec<JoinHandle<()>>) {
        *self.kick_tasks.lock().unwrap() = tasks;
    }

    pub fn stop_kick(&self) {
        for t in std::mem::take(&mut *self.kick_tasks.lock().unwrap()) {
            t.abort();
        }
    }

    #[cfg(test)]
    pub fn test() -> Self {
        Self::new(db::open_in_memory().expect("in-memory db"))
    }
}

/// Coalesced broadcaster: at 100ms cadence emit when state is dirty; tick the
/// countdown once a second (re-emitting so it animates) and persist on resolve.
///
/// Uses `tauri::async_runtime::spawn` (NOT `tokio::spawn`) because it's launched
/// from Tauri's synchronous `.setup()` hook, which is not inside a Tokio runtime —
/// a raw `tokio::spawn` there panics with "there is no reactor running".
pub fn spawn_broadcaster(state: AppState) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        let mut last_sec = std::time::Instant::now();
        loop {
            tick.tick().await;
            let mut send = state.take_dirty();
            if last_sec.elapsed() >= Duration::from_secs(1) {
                last_sec = std::time::Instant::now();
                let (redraw, resolved) = state.tick_battle();
                if redraw {
                    send = true;
                }
                if resolved {
                    state.persist().await;
                }
            }
            if send {
                state.broadcast();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression for the launch panic: spawn_broadcaster runs from Tauri's sync
    // .setup() hook, which has no ambient Tokio runtime. A raw `tokio::spawn` there
    // panics ("there is no reactor running") at the call site — this test would fail
    // if that regressed. The background loop is harmless (no app handle, no battle →
    // it never broadcasts) and dies with the test process.
    #[test]
    fn broadcaster_spawns_without_ambient_runtime() {
        spawn_broadcaster(AppState::test());
    }

    #[test]
    fn oauth_state_round_trips_and_is_single_use() {
        let state = AppState::test();
        state.start_oauth("verifier123".into(), "state-abc".into());
        assert_eq!(
            state.take_oauth("state-abc").as_deref(),
            Some("verifier123")
        );
        // consumed: a second take, even with the right state, is None
        assert!(state.take_oauth("state-abc").is_none());
    }

    #[test]
    fn oauth_state_mismatch_is_rejected_but_preserves_pending_login() {
        let state = AppState::test();
        state.start_oauth("verifier123".into(), "state-abc".into());
        // CSRF check: a wrong `state` param never returns the verifier...
        assert!(state.take_oauth("attacker-guess").is_none());
        // ...and it does NOT burn the real pending login, so the legitimate
        // callback still completes (a bad guess can't DoS an in-flight login).
        assert_eq!(
            state.take_oauth("state-abc").as_deref(),
            Some("verifier123")
        );
    }

    #[test]
    fn oauth_take_without_a_pending_login_is_none() {
        let state = AppState::test();
        assert!(state.take_oauth("anything").is_none());
    }

    #[test]
    fn webhook_ids_dedupe_replays() {
        let state = AppState::test();
        assert!(state.webhook_id_is_new("id-a"), "first sighting is new");
        assert!(!state.webhook_id_is_new("id-a"), "a replay is not new");
        assert!(state.webhook_id_is_new("id-b"), "a different id is new");
    }
}
