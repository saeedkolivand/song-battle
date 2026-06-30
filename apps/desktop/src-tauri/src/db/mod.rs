//! SQLite persistence. Three tables (battles / songs / matches — no per-vote
//! rows). Every mutation rewrites the active battle wholesale; battles are tiny
//! so this stays simple and correct. Runtime-only fields (votes, live timer)
//! are not persisted; an `active` match reloads as `pending` for a clean restart.

use crate::domain::{
    battle::{Battle, BattleStatus},
    bracket::{Match, MatchState},
    song::{Song, Source},
    timer::Timer,
    vote::{Votes, VoteChoice},
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

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(SCHEMA)])
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

/// Replace the stored copy of `b` (preserving its original `created_at`).
pub fn save_battle(conn: &Connection, b: &Battle) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT OR REPLACE INTO battles
         (id,title,description,theme,status,total_rounds,duration_sec,winner_song_id,current_idx,created_at)
         VALUES (?,?,?,?,?,?,?,?,?,
            COALESCE((SELECT created_at FROM battles WHERE id=?), strftime('%s','now')))",
        params![
            b.id, b.title, b.description, b.theme,
            status_str(b.status), b.total_rounds, b.duration_sec,
            b.winner.as_ref().map(|s| &s.id),
            b.current.map(|c| c as i64),
            b.id
        ],
    )?;
    tx.execute("DELETE FROM songs WHERE battle_id=?", params![b.id])?;
    for (i, s) in b.songs.iter().enumerate() {
        tx.execute(
            "INSERT INTO songs
             (id,battle_id,ordering,title,artist,thumbnail,duration_sec,source,source_url,submitter)
             VALUES (?,?,?,?,?,?,?,?,?,?)",
            params![
                s.id, b.id, i as i64, s.title, s.artist, s.thumbnail,
                s.duration_sec, source_str(s.source), s.source_url, s.submitter
            ],
        )?;
    }
    tx.execute("DELETE FROM matches WHERE battle_id=?", params![b.id])?;
    for m in &b.matches {
        tx.execute(
            "INSERT INTO matches (id,battle_id,round,idx,a_song_id,b_song_id,winner,state)
             VALUES (?,?,?,?,?,?,?,?)",
            params![
                m.id, b.id, m.round, m.idx,
                m.a.as_ref().map(|s| &s.id),
                m.b.as_ref().map(|s| &s.id),
                winner_str(m.winner),
                state_str(m.state)
            ],
        )?;
    }
    // Single-active-battle model: prune every other battle (and its rows, incl.
    // submitter usernames) so storage doesn't grow unbounded across saves.
    tx.execute("DELETE FROM songs WHERE battle_id<>?", params![b.id])?;
    tx.execute("DELETE FROM matches WHERE battle_id<>?", params![b.id])?;
    tx.execute("DELETE FROM battles WHERE id<>?", params![b.id])?;
    tx.commit()?;
    Ok(())
}

/// Load the most recently created battle (the one to resume on launch).
pub fn load_latest(conn: &Connection) -> AppResult<Option<Battle>> {
    let row = conn
        .query_row(
            "SELECT id,title,description,theme,status,total_rounds,duration_sec,winner_song_id,current_idx
             FROM battles ORDER BY created_at DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, u32>(5)?,
                    r.get::<_, u32>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((id, title, description, theme, status, total_rounds, duration_sec, winner_id, current_idx)) =
        row
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
        status: status_from(&status),
        songs,
        matches,
        total_rounds,
        current: current_idx.map(|c| c as usize),
        winner,
        duration_sec,
    };
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
    let mut stmt = conn.prepare(
        "SELECT id,round,idx,a_song_id,b_song_id,winner,state
         FROM matches WHERE battle_id=? ORDER BY round, idx",
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
            round: r.get(1)?,
            idx: r.get(2)?,
            a: a_id.and_then(|i| by_id.get(i.as_str()).map(|s| (*s).clone())),
            b: b_id.and_then(|i| by_id.get(i.as_str()).map(|s| (*s).clone())),
            votes: Votes::default(),
            state,
            winner: winner_from(winner.as_deref()),
            timer: Timer::new(duration_sec),
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
        b.generate_bracket().unwrap();
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
    fn saving_a_new_battle_prunes_the_previous_one() {
        let conn = open_in_memory().unwrap();
        let first = sample();
        save_battle(&conn, &first).unwrap();

        let second = Battle::new("Second".into(), String::new(), String::new());
        save_battle(&conn, &second).unwrap();

        // Only the latest battle (no songs/matches/winner) remains.
        let battles: i64 = conn
            .query_row("SELECT COUNT(*) FROM battles", [], |r| r.get(0))
            .unwrap();
        let songs: i64 = conn
            .query_row("SELECT COUNT(*) FROM songs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(battles, 1);
        assert_eq!(songs, 0);
        assert_eq!(load_latest(&conn).unwrap().unwrap().id, second.id);
    }

    #[test]
    fn out_of_range_current_is_clamped_on_load() {
        let conn = open_in_memory().unwrap();
        let mut b = Battle::new("t".into(), String::new(), String::new());
        b.current = Some(99); // malformed pointer, no matches
        save_battle(&conn, &b).unwrap();
        assert_eq!(load_latest(&conn).unwrap().unwrap().current, None);
    }
}
