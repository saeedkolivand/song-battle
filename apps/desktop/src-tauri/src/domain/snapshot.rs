//! The wire DTOs (must match `packages/types`) and the pure builders that
//! project the battle aggregate into a `Snapshot`.

use crate::domain::{
    battle::{Battle, BattleMode, BattleStatus},
    bracket::{Match, MatchGroup, MatchState},
    song::Song,
    vote::{self, VoteChoice},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerView {
    pub duration_sec: u32,
    pub remaining_sec: u32,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchView {
    pub id: String,
    pub round: u32,
    pub a: Option<Song>,
    pub b: Option<Song>,
    pub votes_a: u32,
    pub votes_b: u32,
    pub pct_a: u32,
    pub pct_b: u32,
    pub total: u32,
    pub state: MatchState,
    pub winner: Option<VoteChoice>,
    pub timer: Option<TimerView>,
    pub group: MatchGroup,
    pub best_of: u32,
    pub wins_a: u32,
    pub wins_b: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleView {
    pub id: String,
    pub title: String,
    pub description: String,
    pub theme: String,
    pub mode: BattleMode,
    pub status: BattleStatus,
    pub round: u32,
    pub total_rounds: u32,
    pub current_match: Option<MatchView>,
    pub bracket: Vec<MatchView>,
    pub winner: Option<Song>,
    pub songs: Vec<Song>,
    pub song_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KickView {
    pub state: ConnectionState,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub seq: u64,
    pub battle: Option<BattleView>,
    pub kick: KickView,
    /// When true, overlay/dashboard hide voter identities (from `Settings`).
    pub anonymous: bool,
}

/// Persisted app settings (single row). `get_settings` returns this shape.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub anonymous: bool,
    pub default_timer_sec: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            anonymous: false,
            default_timer_sec: 30,
        }
    }
}

/// Summary row for the saved-tournaments list (`list_battles`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedBattle {
    pub id: String,
    pub title: String,
    pub theme: String,
    pub status: BattleStatus,
    pub song_count: u32,
    pub updated_at: i64,
}

fn match_view(m: &Match) -> MatchView {
    let (votes_a, votes_b) = m.votes.tally();
    let total = votes_a + votes_b;
    let timer = (m.state == MatchState::Active).then_some(TimerView {
        duration_sec: m.timer.duration_sec,
        remaining_sec: m.timer.remaining_sec,
        running: m.timer.running,
    });
    MatchView {
        id: m.id.clone(),
        round: m.round,
        a: m.a.clone(),
        b: m.b.clone(),
        votes_a,
        votes_b,
        pct_a: vote::pct(votes_a, total),
        pct_b: vote::pct(votes_b, total),
        total,
        state: m.state,
        winner: m.winner,
        timer,
        group: m.group,
        best_of: m.best_of,
        wins_a: m.wins_a,
        wins_b: m.wins_b,
    }
}

/// Drop submitter identity from a song slot (anonymous mode).
fn redact(song: &mut Option<Song>) {
    if let Some(s) = song {
        s.submitter = None;
    }
}

pub fn battle_view(b: &Battle, anonymous: bool) -> BattleView {
    let mut bracket: Vec<MatchView> = b.matches.iter().map(match_view).collect();
    let mut current_match = b.current.and_then(|i| b.matches.get(i)).map(match_view);
    let round = current_match.as_ref().map_or(b.total_rounds, |m| m.round);
    let mut songs = b.songs.clone();
    let mut winner = b.winner.clone();

    // Anonymous mode strips submitter identity server-side, so PII never crosses
    // the WS — not merely hidden client-side.
    if anonymous {
        for m in &mut bracket {
            redact(&mut m.a);
            redact(&mut m.b);
        }
        if let Some(m) = &mut current_match {
            redact(&mut m.a);
            redact(&mut m.b);
        }
        for s in &mut songs {
            s.submitter = None;
        }
        redact(&mut winner);
    }

    BattleView {
        id: b.id.clone(),
        title: b.title.clone(),
        description: b.description.clone(),
        theme: b.theme.clone(),
        mode: b.mode,
        status: b.status,
        round,
        total_rounds: b.total_rounds,
        current_match,
        bracket,
        winner,
        songs,
        song_count: b.songs.len() as u32,
    }
}
