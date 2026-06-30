//! The single source of truth. `AppState` is cloneable (all `Arc`) and shared
//! between Tauri commands, the axum overlay server, and the coalesced
//! broadcaster. Mutations mark a dirty flag + persist; the broadcaster turns
//! dirtiness into one `Snapshot` per tick, fanned to overlay (WS) and dashboard.

use crate::db;
use crate::domain::{
    battle::Battle,
    snapshot::{battle_view, ConnectionState, KickView, SavedBattle, Settings, Snapshot},
    vote::VoteChoice,
};
use crate::error::{AppError, AppResult};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

/// Live in-memory state behind the lock.
#[derive(Default)]
pub struct App {
    pub battle: Option<Battle>,
    pub kick: KickConn,
    pub settings: Settings,
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

#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<App>>,
    tx: broadcast::Sender<String>,
    db: Arc<Mutex<rusqlite::Connection>>,
    seq: Arc<AtomicU64>,
    dirty: Arc<AtomicBool>,
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
    kick_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
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
        }
    }

    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
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
pub fn spawn_broadcaster(state: AppState) {
    tokio::spawn(async move {
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
