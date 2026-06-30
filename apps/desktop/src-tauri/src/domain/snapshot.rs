//! The wire DTOs (must match `packages/types`) and the pure builders that
//! project the battle aggregate into a `Snapshot`.

use crate::domain::{
    battle::{Battle, BattleStatus},
    bracket::{Match, MatchState},
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleView {
    pub id: String,
    pub title: String,
    pub description: String,
    pub theme: String,
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
    }
}

pub fn battle_view(b: &Battle) -> BattleView {
    let bracket: Vec<MatchView> = b.matches.iter().map(match_view).collect();
    let current_match = b.current.and_then(|i| b.matches.get(i)).map(match_view);
    let round = current_match
        .as_ref()
        .map_or(b.total_rounds, |m| m.round);
    BattleView {
        id: b.id.clone(),
        title: b.title.clone(),
        description: b.description.clone(),
        theme: b.theme.clone(),
        status: b.status,
        round,
        total_rounds: b.total_rounds,
        current_match,
        bracket,
        winner: b.winner.clone(),
        songs: b.songs.clone(),
        song_count: b.songs.len() as u32,
    }
}
