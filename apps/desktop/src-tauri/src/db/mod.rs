//! SQLite persistence. Three tables (battles / songs / matches — no per-vote
//! rows). Every mutation rewrites the active battle wholesale; battles are tiny
//! so this stays simple and correct. Runtime-only fields (votes, live timer)
//! are not persisted; an `active` match reloads as `pending` for a clean restart.

use crate::domain::{
    battle::{Battle, BattleMode, BattleStatus},
    bracket::{Match, MatchGroup, MatchState},
    snapshot::{SavedBattle, Settings},
    song::{Song, Source},
    timer::Timer,
    vote::{VoteChoice, Votes},
};
use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use std::collections::HashMap;
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE battles (
  id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT NOT NULL, theme TEXT NOT NULL,
  status TEXT NOT NULL, total_rounds INTEGER NOT NULL, duration_sec INTEGER NOT NULL,
  winner_song_id TEXT, current_idx INTEGER, created_at INTEGER NOT NULL
);
CREATE TABLE songs (
  id TEXT PRIMARY KEY, battle_id TEXT NOT NULL, ordering INTEGER NOT NULL, title TEXT NOT NULL,
  artist TEXT, thumbnail TEXT, duration_sec INTEGER, source TEXT NOT NULL,
  source_url TEXT NOT NULL, submitter TEXT
);
CREATE TABLE matches (
  id TEXT PRIMARY KEY, battle_id TEXT NOT NULL, round INTEGER NOT NULL, idx INTEGER NOT NULL,
  a_song_id TEXT, b_song_id TEXT, winner TEXT, state TEXT NOT NULL
);
";

// Phase 2: keep many battles (newest-first ordering) + persisted settings.
const M2_UPDATED_AT: &str = "ALTER TABLE battles ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;";
const M3_SETTINGS: &str = "
CREATE TABLE settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  anonymous INTEGER NOT NULL,
  default_timer_sec INTEGER NOT NULL
);
INSERT INTO settings (id, anonymous, default_timer_sec) VALUES (1, 0, 30);
";
// Phase 3: tournament modes. `mode` on battles; group/series fields on matches.
// (`match_group` avoids the `group` SQL keyword.) Routing is recomputed on load.
const M4_MODES: &str = "
ALTER TABLE battles ADD COLUMN mode TEXT NOT NULL DEFAULT 'single';
ALTER TABLE matches ADD COLUMN match_group TEXT NOT NULL DEFAULT 'main';
ALTER TABLE matches ADD COLUMN best_of INTEGER NOT NULL DEFAULT 1;
ALTER TABLE matches ADD COLUMN wins_a INTEGER NOT NULL DEFAULT 0;
ALTER TABLE matches ADD COLUMN wins_b INTEGER NOT NULL DEFAULT 0;
";
// Chat song submissions toggle (default on).
const M5_CHAT_SUBMISSIONS: &str =
    "ALTER TABLE settings ADD COLUMN chat_submissions INTEGER NOT NULL DEFAULT 1;";
// K1: official Kick API OAuth 2.1 + PKCE creds/tokens. Single row like `settings`.
// Everything but `id` is nullable — unset until the user logs in.
const M6_KICK_AUTH: &str = "
CREATE TABLE kick_auth (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  client_id TEXT,
  client_secret TEXT,
  access_token TEXT,
  refresh_token TEXT,
  expires_at INTEGER,
  subscription_id TEXT
);
INSERT INTO kick_auth (id) VALUES (1);
";

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(SCHEMA),
        M::up(M2_UPDATED_AT),
        M::up(M3_SETTINGS),
        M::up(M4_MODES),
        M::up(M5_CHAT_SUBMISSIONS),
        M::up(M6_KICK_AUTH),
    ])
}

/// Unix time in ms (used for the battles `updated_at` column).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> AppResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

/// Replace the stored copy of `b` (preserving its original `created_at`, bumping
/// `updated_at`). Other battles are kept — Phase 2 supports many saved tournaments.
pub fn save_battle(conn: &Connection, b: &Battle) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT OR REPLACE INTO battles
         (id,title,description,theme,mode,status,total_rounds,duration_sec,winner_song_id,current_idx,created_at,updated_at)
         VALUES (?,?,?,?,?,?,?,?,?,?,
            COALESCE((SELECT created_at FROM battles WHERE id=?), strftime('%s','now')), ?)",
        params![
            b.id, b.title, b.description, b.theme, mode_str(b.mode),
            status_str(b.status), b.total_rounds, b.duration_sec,
            b.winner.as_ref().map(|s| &s.id),
            b.current.map(|c| c as i64),
            b.id, now_ms()
        ],
    )?;
    tx.execute("DELETE FROM songs WHERE battle_id=?", params![b.id])?;
    for (i, s) in b.songs.iter().enumerate() {
        tx.execute(
            "INSERT INTO songs
             (id,battle_id,ordering,title,artist,thumbnail,duration_sec,source,source_url,submitter)
             VALUES (?,?,?,?,?,?,?,?,?,?)",
            params![
                s.id,
                b.id,
                i as i64,
                s.title,
                s.artist,
                s.thumbnail,
                s.duration_sec,
                source_str(s.source),
                s.source_url,
                s.submitter
            ],
        )?;
    }
    tx.execute("DELETE FROM matches WHERE battle_id=?", params![b.id])?;
    for m in &b.matches {
        tx.execute(
            "INSERT INTO matches
             (id,battle_id,round,idx,a_song_id,b_song_id,winner,state,match_group,best_of,wins_a,wins_b)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                m.id, b.id, m.round, m.idx,
                m.a.as_ref().map(|s| &s.id),
                m.b.as_ref().map(|s| &s.id),
                winner_str(m.winner),
                state_str(m.state),
                group_str(m.group), m.best_of, m.wins_a, m.wins_b
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Saved-tournament summaries, newest-first by `updated_at`.
pub fn list_battles(conn: &Connection) -> AppResult<Vec<SavedBattle>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.title, b.theme, b.status, b.updated_at,
                (SELECT COUNT(*) FROM songs WHERE battle_id = b.id)
         FROM battles b ORDER BY b.updated_at DESC, b.rowid DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(SavedBattle {
            id: r.get(0)?,
            title: r.get(1)?,
            theme: r.get(2)?,
            status: status_from(&r.get::<_, String>(3)?),
            updated_at: r.get(4)?,
            song_count: r.get::<_, i64>(5)? as u32,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Delete a battle and all its songs/matches.
pub fn delete_battle(conn: &Connection, id: &str) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM songs WHERE battle_id=?", params![id])?;
    tx.execute("DELETE FROM matches WHERE battle_id=?", params![id])?;
    tx.execute("DELETE FROM battles WHERE id=?", params![id])?;
    tx.commit()?;
    Ok(())
}

// ── settings ─────────────────────────────────────────────────────────────────
pub fn get_settings(conn: &Connection) -> AppResult<Settings> {
    Ok(conn.query_row(
        "SELECT anonymous, default_timer_sec, chat_submissions FROM settings WHERE id=1",
        [],
        |r| {
            Ok(Settings {
                anonymous: r.get::<_, i64>(0)? != 0,
                default_timer_sec: r.get::<_, u32>(1)?,
                chat_submissions: r.get::<_, i64>(2)? != 0,
            })
        },
    )?)
}

pub fn set_anonymous(conn: &Connection, anonymous: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE settings SET anonymous=? WHERE id=1",
        params![anonymous as i64],
    )?;
    Ok(())
}

pub fn set_default_timer(conn: &Connection, sec: u32) -> AppResult<()> {
    conn.execute(
        "UPDATE settings SET default_timer_sec=? WHERE id=1",
        params![sec],
    )?;
    Ok(())
}

pub fn set_chat_submissions(conn: &Connection, enabled: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE settings SET chat_submissions=? WHERE id=1",
        params![enabled as i64],
    )?;
    Ok(())
}

// ── official Kick OAuth (K1) ────────────────────────────────────────────────
// ponytail: plaintext client_secret/tokens in SQLite, same as the OBS password
// — local desktop app, not a hosted service. Flagged for the security review.
#[derive(Debug, Clone, Default)]
pub struct KickAuth {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub access_token: Option<String>,
    // K2 reads these (token-refresh check); K1 only round-trips them through
    // the DB (write in set_kick_tokens, exercised by db::tests::kick_auth_round_trips).
    #[allow(dead_code)]
    pub refresh_token: Option<String>,
    #[allow(dead_code)]
    pub expires_at: Option<i64>,
    pub subscription_id: Option<String>,
}

pub fn get_kick_auth(conn: &Connection) -> AppResult<KickAuth> {
    Ok(conn.query_row(
        "SELECT client_id, client_secret, access_token, refresh_token, expires_at, subscription_id
         FROM kick_auth WHERE id=1",
        [],
        |r| {
            Ok(KickAuth {
                client_id: r.get(0)?,
                client_secret: r.get(1)?,
                access_token: r.get(2)?,
                refresh_token: r.get(3)?,
                expires_at: r.get(4)?,
                subscription_id: r.get(5)?,
            })
        },
    )?)
}

pub fn set_kick_creds(conn: &Connection, client_id: &str, client_secret: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE kick_auth SET client_id=?, client_secret=? WHERE id=1",
        params![client_id, client_secret],
    )?;
    Ok(())
}

/// `refresh_token` is only overwritten when `Some` — Kick doesn't always
/// rotate it on `grant_type=refresh_token`, so `None` keeps the existing one.
pub fn set_kick_tokens(
    conn: &Connection,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at: i64,
) -> AppResult<()> {
    match refresh_token {
        Some(rt) => conn.execute(
            "UPDATE kick_auth SET access_token=?, refresh_token=?, expires_at=? WHERE id=1",
            params![access_token, rt, expires_at],
        ),
        None => conn.execute(
            "UPDATE kick_auth SET access_token=?, expires_at=? WHERE id=1",
            params![access_token, expires_at],
        ),
    }?;
    Ok(())
}

/// Persist (or clear) the active webhook subscription id.
pub fn set_kick_subscription(conn: &Connection, subscription_id: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE kick_auth SET subscription_id=? WHERE id=1",
        params![subscription_id],
    )?;
    Ok(())
}

/// Local logout: clears creds, tokens, and subscription id. (K2 also deletes
/// the remote webhook subscription before calling this.)
pub fn clear_kick_auth(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE kick_auth SET client_id=NULL, client_secret=NULL, access_token=NULL,
         refresh_token=NULL, expires_at=NULL, subscription_id=NULL WHERE id=1",
        [],
    )?;
    Ok(())
}

/// Load the most-recently-updated battle (the one to resume on launch).
pub fn load_latest(conn: &Connection) -> AppResult<Option<Battle>> {
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM battles ORDER BY updated_at DESC, rowid DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    match id {
        Some(id) => load_battle(conn, &id),
        None => Ok(None),
    }
}

/// Load a specific battle by id, normalized for boot.
pub fn load_battle(conn: &Connection, id: &str) -> AppResult<Option<Battle>> {
    let row = conn
        .query_row(
            "SELECT id,title,description,theme,mode,status,total_rounds,duration_sec,winner_song_id,current_idx
             FROM battles WHERE id=?",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, u32>(6)?,
                    r.get::<_, u32>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, Option<i64>>(9)?,
                ))
            },
        )
        .optional()?;
    let Some((
        id,
        title,
        description,
        theme,
        mode,
        status,
        total_rounds,
        duration_sec,
        winner_id,
        current_idx,
    )) = row
    else {
        return Ok(None);
    };

    let songs = load_songs(conn, &id)?;
    let by_id: HashMap<&str, &Song> = songs.iter().map(|s| (s.id.as_str(), s)).collect();
    let matches = load_matches(conn, &id, &by_id, duration_sec)?;
    let winner = winner_id.and_then(|w| songs.iter().find(|s| s.id == w).cloned());

    let mut battle = Battle {
        id,
        title,
        description,
        theme,
        mode: mode_from(&mode),
        status: status_from(&status),
        songs,
        matches,
        total_rounds,
        current: current_idx.map(|c| c as usize),
        winner,
        duration_sec,
    };
    battle.rewire(); // recompute routing + fill flags (not persisted)
    battle.normalize(); // untrusted boundary: clamp out-of-range `current`
    Ok(Some(battle))
}

fn load_songs(conn: &Connection, battle_id: &str) -> AppResult<Vec<Song>> {
    let mut stmt = conn.prepare(
        "SELECT id,title,artist,thumbnail,duration_sec,source,source_url,submitter
         FROM songs WHERE battle_id=? ORDER BY ordering",
    )?;
    let rows = stmt.query_map(params![battle_id], |r| {
        Ok(Song {
            id: r.get(0)?,
            title: r.get(1)?,
            artist: r.get(2)?,
            thumbnail: r.get(3)?,
            duration_sec: r.get(4)?,
            source: source_from(&r.get::<_, String>(5)?),
            source_url: r.get(6)?,
            submitter: r.get(7)?,
            metadata: None,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn load_matches(
    conn: &Connection,
    battle_id: &str,
    by_id: &HashMap<&str, &Song>,
    duration_sec: u32,
) -> AppResult<Vec<Match>> {
    // Order MUST mirror generate_double's build order (all winners/main, then losers,
    // then grand; each by round, idx) — `current` is a persisted positional index into
    // this vector, so a different load order would point it at the wrong match.
    let mut stmt = conn.prepare(
        "SELECT id,round,idx,a_song_id,b_song_id,winner,state,match_group,best_of,wins_a,wins_b
         FROM matches WHERE battle_id=?
         ORDER BY CASE match_group WHEN 'losers' THEN 1 WHEN 'grand' THEN 2 ELSE 0 END, round, idx",
    )?;
    let rows = stmt.query_map(params![battle_id], |r| {
        let a_id: Option<String> = r.get(3)?;
        let b_id: Option<String> = r.get(4)?;
        let winner: Option<String> = r.get(5)?;
        let state = state_from(&r.get::<_, String>(6)?);
        // a previously-active match restarts cleanly (live timer isn't persisted)
        let state = if state == MatchState::Active {
            MatchState::Pending
        } else {
            state
        };
        Ok(Match {
            id: r.get(0)?,
            group: group_from(&r.get::<_, String>(7)?),
            round: r.get(1)?,
            idx: r.get(2)?,
            a: a_id.and_then(|i| by_id.get(i.as_str()).map(|s| (*s).clone())),
            b: b_id.and_then(|i| by_id.get(i.as_str()).map(|s| (*s).clone())),
            votes: Votes::default(),
            state,
            winner: winner_from(winner.as_deref()),
            timer: Timer::new(duration_sec),
            best_of: r.get(8)?,
            wins_a: r.get(9)?,
            wins_b: r.get(10)?,
            // recomputed by Battle::rewire after the whole battle is built
            win_to: None,
            lose_to: None,
            a_filled: false,
            b_filled: false,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// ── enum <-> text ───────────────────────────────────────────────────────────
fn source_str(s: Source) -> &'static str {
    match s {
        Source::Youtube => "youtube",
        Source::Spotify => "spotify",
        Source::Soundcloud => "soundcloud",
    }
}
fn source_from(s: &str) -> Source {
    match s {
        "spotify" => Source::Spotify,
        "soundcloud" => Source::Soundcloud,
        _ => Source::Youtube,
    }
}
fn status_str(s: BattleStatus) -> &'static str {
    match s {
        BattleStatus::Idle => "idle",
        BattleStatus::Running => "running",
        BattleStatus::Finished => "finished",
    }
}
fn status_from(s: &str) -> BattleStatus {
    match s {
        "running" => BattleStatus::Running,
        "finished" => BattleStatus::Finished,
        _ => BattleStatus::Idle,
    }
}
fn mode_str(m: BattleMode) -> &'static str {
    match m {
        BattleMode::Single => "single",
        BattleMode::Double => "double",
        BattleMode::Bo3 => "bo3",
    }
}
fn mode_from(s: &str) -> BattleMode {
    match s {
        "double" => BattleMode::Double,
        "bo3" => BattleMode::Bo3,
        _ => BattleMode::Single,
    }
}
fn group_str(g: MatchGroup) -> &'static str {
    match g {
        MatchGroup::Main => "main",
        MatchGroup::Winners => "winners",
        MatchGroup::Losers => "losers",
        MatchGroup::Grand => "grand",
    }
}
fn group_from(s: &str) -> MatchGroup {
    match s {
        "winners" => MatchGroup::Winners,
        "losers" => MatchGroup::Losers,
        "grand" => MatchGroup::Grand,
        _ => MatchGroup::Main,
    }
}
fn state_str(s: MatchState) -> &'static str {
    match s {
        MatchState::Pending => "pending",
        MatchState::Active => "active",
        MatchState::Done => "done",
    }
}
fn state_from(s: &str) -> MatchState {
    match s {
        "active" => MatchState::Active,
        "done" => MatchState::Done,
        _ => MatchState::Pending,
    }
}
fn winner_str(w: Option<VoteChoice>) -> Option<&'static str> {
    w.map(|c| match c {
        VoteChoice::A => "a",
        VoteChoice::B => "b",
    })
}
fn winner_from(s: Option<&str>) -> Option<VoteChoice> {
    match s {
        Some("a") => Some(VoteChoice::A),
        Some("b") => Some(VoteChoice::B),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Battle {
        let mut b = Battle::new("My Battle".into(), "desc".into(), "synthwave".into());
        for i in 0..4 {
            b.add_song(Song {
                id: format!("s{i}"),
                title: format!("song {i}"),
                artist: Some(format!("artist {i}")),
                thumbnail: None,
                duration_sec: Some(120),
                source: Source::Youtube,
                source_url: format!("https://x/{i}"),
                submitter: Some("bob".into()),
                metadata: None,
            });
        }
        b.generate_bracket(BattleMode::Single).unwrap();
        b
    }

    #[test]
    fn round_trips_through_sqlite() {
        let conn = open_in_memory().unwrap();
        let b = sample();
        save_battle(&conn, &b).unwrap();
        let loaded = load_latest(&conn).unwrap().expect("a battle");
        assert_eq!(loaded.id, b.id);
        assert_eq!(loaded.title, "My Battle");
        assert_eq!(loaded.songs.len(), 4);
        assert_eq!(loaded.matches.len(), b.matches.len());
        assert_eq!(loaded.total_rounds, b.total_rounds);
        assert_eq!(loaded.songs[2].artist.as_deref(), Some("artist 2"));
    }

    #[test]
    fn save_is_idempotent_and_keeps_latest() {
        let conn = open_in_memory().unwrap();
        let mut b = sample();
        save_battle(&conn, &b).unwrap();
        b.title = "Renamed".into();
        save_battle(&conn, &b).unwrap();
        let loaded = load_latest(&conn).unwrap().unwrap();
        assert_eq!(loaded.title, "Renamed");
        // no duplicate rows
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM songs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 4);
    }

    #[test]
    fn empty_db_loads_none() {
        let conn = open_in_memory().unwrap();
        assert!(load_latest(&conn).unwrap().is_none());
    }

    #[test]
    fn out_of_range_current_is_clamped_on_load() {
        let conn = open_in_memory().unwrap();
        let mut b = Battle::new("t".into(), String::new(), String::new());
        b.current = Some(99); // malformed pointer, no matches
        save_battle(&conn, &b).unwrap();
        assert_eq!(load_latest(&conn).unwrap().unwrap().current, None);
    }

    #[test]
    fn multi_battle_list_load_delete_round_trip() {
        let conn = open_in_memory().unwrap();
        let first = sample(); // 4 songs
        save_battle(&conn, &first).unwrap();
        let second = Battle::new("Second".into(), "d".into(), "vapor".into());
        save_battle(&conn, &second).unwrap();

        // Both kept; newest (second) first.
        let list = list_battles(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, second.id);
        assert_eq!(list[0].song_count, 0);
        assert_eq!(list[1].id, first.id);
        assert_eq!(list[1].song_count, 4);
        assert_eq!(list[1].theme, "synthwave");
        assert!(list[0].updated_at >= list[1].updated_at);

        // Load a specific one.
        let loaded = load_battle(&conn, &first.id).unwrap().unwrap();
        assert_eq!(loaded.songs.len(), 4);
        assert!(load_battle(&conn, "nope").unwrap().is_none());

        // Delete removes it + its songs, leaving the other.
        delete_battle(&conn, &first.id).unwrap();
        let list = list_battles(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, second.id);
        let orphan_songs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM songs WHERE battle_id=?",
                params![first.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan_songs, 0);
    }

    #[test]
    fn kick_auth_round_trips() {
        let conn = open_in_memory().unwrap();
        // migration seeds an all-NULL row
        let auth = get_kick_auth(&conn).unwrap();
        assert!(auth.client_id.is_none());
        assert!(auth.access_token.is_none());

        set_kick_creds(&conn, "cid", "secret").unwrap();
        set_kick_tokens(&conn, "AT1", Some("RT1"), 1_700_000_000).unwrap();
        let auth = get_kick_auth(&conn).unwrap();
        assert_eq!(auth.client_id.as_deref(), Some("cid"));
        assert_eq!(auth.client_secret.as_deref(), Some("secret"));
        assert_eq!(auth.access_token.as_deref(), Some("AT1"));
        assert_eq!(auth.refresh_token.as_deref(), Some("RT1"));
        assert_eq!(auth.expires_at, Some(1_700_000_000));

        // a refresh that didn't rotate the refresh token must keep the old one
        set_kick_tokens(&conn, "AT2", None, 1_700_000_999).unwrap();
        let auth = get_kick_auth(&conn).unwrap();
        assert_eq!(auth.access_token.as_deref(), Some("AT2"));
        assert_eq!(
            auth.refresh_token.as_deref(),
            Some("RT1"),
            "unrotated refresh token is kept"
        );

        set_kick_subscription(&conn, Some("sub-1")).unwrap();
        assert_eq!(
            get_kick_auth(&conn).unwrap().subscription_id.as_deref(),
            Some("sub-1")
        );

        clear_kick_auth(&conn).unwrap();
        let auth = get_kick_auth(&conn).unwrap();
        assert!(auth.client_id.is_none());
        assert!(auth.client_secret.is_none());
        assert!(auth.access_token.is_none());
        assert!(auth.refresh_token.is_none());
        assert!(auth.expires_at.is_none());
        assert!(auth.subscription_id.is_none());
    }

    #[test]
    fn settings_persist() {
        let conn = open_in_memory().unwrap();
        // migration seeds defaults
        let s = get_settings(&conn).unwrap();
        assert!(!s.anonymous);
        assert_eq!(s.default_timer_sec, 30);
        assert!(s.chat_submissions); // default on

        set_anonymous(&conn, true).unwrap();
        set_default_timer(&conn, 45).unwrap();
        set_chat_submissions(&conn, false).unwrap();
        let s = get_settings(&conn).unwrap();
        assert!(s.anonymous);
        assert_eq!(s.default_timer_sec, 45);
        assert!(!s.chat_submissions);
    }

    /// REGRESSION for the CRITICAL load-ordering bug fixed in load_matches.
    ///
    /// The bug: without `ORDER BY CASE match_group …` the DB returned matches
    /// ordered by (round, idx) only, which interleaved Winners / Losers / Grand
    /// rows and produced a different positional vector than generate_double builds.
    /// A saved `current_idx = 2` would point at the wrong match (Grand-r1 or
    /// LB-r1) instead of WB-r2 after reload.
    ///
    /// The fix: ORDER BY CASE …group… END, round, idx mirrors generate order
    /// exactly. This test saves a mid-tournament 4-song double-elim battle with
    /// current pointing to WB-r2 (index 2), reloads it, and asserts:
    ///   • the loaded `current` index is still 2
    ///   • the match at index 2 has the same ID as before (not LB-r1 or Grand-r1)
    ///   • start_match() activates that exact match (correct first_playable)
    #[test]
    fn double_elim_reload_preserves_current_mid_tourney() {
        let conn = open_in_memory().unwrap();

        // Build a 4-song double-elim battle (timer=2s so two ticks expire it).
        let mut b = Battle::new("reload-test".into(), "d".into(), "th".into());
        for i in 0..4_u32 {
            b.add_song(Song {
                id: format!("s{i}"),
                title: format!("song {i}"),
                artist: None,
                thumbnail: None,
                duration_sec: None,
                source: Source::Youtube,
                source_url: format!("https://x/{i}"),
                submitter: None,
                metadata: None,
            });
        }
        b.set_timer(2);
        b.generate_bracket(BattleMode::Double).unwrap();

        // generate order for 4-song double-elim:
        //   idx 0: Winners r1 idx0 (s0 vs s3)
        //   idx 1: Winners r1 idx1 (s1 vs s2)
        //   idx 2: Winners r2 idx0  ← target current after two WB-r1 matches
        //   idx 3: Losers  r1 idx0
        //   idx 4: Losers  r2 idx0
        //   idx 5: Grand   r1 idx0
        //   idx 6: Grand   r2 idx0

        // Play WB-r1-idx0: A wins → s0 to WB-r2 slot-a, s3 to LB-r1 slot-a.
        b.start_match().unwrap();
        assert_eq!(b.current, Some(0));
        b.cast_vote("u1".into(), VoteChoice::A, 1_000);
        b.tick();
        b.tick(); // 2s timer expires → match resolves
        assert_eq!(b.matches[0].state, MatchState::Done);

        // Play WB-r1-idx1: A wins → s1 to WB-r2 slot-b, s2 to LB-r1 slot-b.
        b.start_match().unwrap();
        assert_eq!(b.current, Some(1));
        b.cast_vote("u2".into(), VoteChoice::A, 2_000);
        b.tick();
        b.tick();
        assert_eq!(b.matches[1].state, MatchState::Done);

        // Both WB-r2 slots are filled; LB-r1 is also ready, but WB-r2 is earlier
        // in the vector → first_playable = 2 = WB-r2.
        assert_eq!(b.current, Some(2));
        assert_eq!(b.matches[2].group, MatchGroup::Winners);
        assert_eq!(b.matches[2].round, 2);
        let expected_match_id = b.matches[2].id.clone();

        // Persist and reload.
        save_battle(&conn, &b).unwrap();
        let loaded = load_latest(&conn)
            .unwrap()
            .expect("battle present after save");

        // ── assertions that catch the load-ordering bug ──────────────────────
        assert_eq!(
            loaded.current,
            Some(2),
            "current index must survive the round-trip"
        );
        assert_eq!(
            loaded.matches[2].id, expected_match_id,
            "index 2 must be the WB-r2 match, not LB-r1 or Grand-r1 (load-order regression)"
        );
        assert_eq!(loaded.matches[2].group, MatchGroup::Winners);
        assert_eq!(loaded.matches[2].round, 2);

        // start_match uses first_playable() to find the next Pending match with
        // both songs present; it must land on the same WB-r2 match.
        let mut loaded = loaded;
        loaded.start_match().unwrap();
        assert_eq!(
            loaded.current,
            Some(2),
            "start_match must activate WB-r2 (not LB-r1)"
        );
        assert_eq!(loaded.matches[2].state, MatchState::Active);
        assert_eq!(loaded.matches[2].id, expected_match_id);
    }

    #[test]
    fn double_elim_mode_and_routing_round_trip() {
        let conn = open_in_memory().unwrap();
        let mut b = sample(); // 4 songs
        b.generate_bracket(BattleMode::Double).unwrap();
        save_battle(&conn, &b).unwrap();

        let loaded = load_latest(&conn).unwrap().unwrap();
        assert_eq!(loaded.mode, BattleMode::Double);
        assert_eq!(loaded.matches.len(), b.matches.len()); // 7
                                                           // mode/group/best_of persisted
        assert!(loaded.matches.iter().any(|m| m.group == MatchGroup::Grand));
        // routing was recomputed on load (not persisted): WB final → grand slot a.
        let wf = loaded
            .matches
            .iter()
            .find(|m| m.group == MatchGroup::Winners && m.round == 2)
            .unwrap();
        assert_eq!(wf.win_to.unwrap().group, MatchGroup::Grand);
        assert!(wf.win_to.unwrap().slot_a);
    }
}
