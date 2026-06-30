//! Single-elimination bracket. Seeds are taken in the songs' current order
//! (call `Battle::shuffle` first for random seeding — keeps this pure/testable).
//! Byes are given to top seeds and auto-resolved at generation time.

use crate::domain::{
    song::Song,
    timer::Timer,
    vote::{VoteChoice, Votes},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchState {
    Pending,
    Active,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub id: String,
    pub round: u32, // 1-based
    pub idx: u32,   // 0-based index within the round
    pub a: Option<Song>,
    pub b: Option<Song>,
    #[serde(default)]
    pub votes: Votes,
    pub state: MatchState,
    pub winner: Option<VoteChoice>,
    pub timer: Timer,
}

impl Match {
    fn new(round: u32, idx: u32, a: Option<Song>, b: Option<Song>, duration_sec: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            round,
            idx,
            a,
            b,
            votes: Votes::default(),
            state: MatchState::Pending,
            winner: None,
            timer: Timer::new(duration_sec),
        }
    }
}

/// `ceil(log2(n))`; 0 for n < 2.
pub fn total_rounds(n: usize) -> u32 {
    if n < 2 {
        return 0;
    }
    let mut rounds = 0;
    let mut size = 1usize;
    while size < n {
        size <<= 1;
        rounds += 1;
    }
    rounds
}

/// The match a winner of `(round, idx)` feeds into: `(round, idx, fills_slot_a)`.
pub fn parent(round: u32, idx: u32) -> (u32, u32, bool) {
    (round + 1, idx / 2, idx.is_multiple_of(2))
}

/// Build every round up front. Round 1 is seeded `seed[i]` vs `seed[slots-1-i]`
/// so byes land on the highest seeds; later rounds start empty.
pub fn generate(songs: &[Song], duration_sec: u32) -> Vec<Match> {
    let n = songs.len();
    let tr = total_rounds(n);
    if tr == 0 {
        return Vec::new();
    }
    let slots = 1usize << tr;
    let mut matches = Vec::new();

    for m in 0..slots / 2 {
        let a = songs.get(m).cloned();
        let b = songs.get(slots - 1 - m).cloned();
        matches.push(Match::new(1, m as u32, a, b, duration_sec));
    }
    for round in 2..=tr {
        let count = slots >> round; // 2^(tr-round)
        for m in 0..count {
            matches.push(Match::new(round, m as u32, None, None, duration_sec));
        }
    }

    resolve_byes(&mut matches, tr);
    matches
}

/// A round-1 slot with exactly one song wins for free and advances immediately.
fn resolve_byes(matches: &mut [Match], tr: u32) {
    let mut advance: Vec<(u32, u32, bool, Song)> = Vec::new();
    for m in matches.iter_mut().filter(|m| m.round == 1) {
        let win = match (&m.a, &m.b) {
            (Some(s), None) => Some((VoteChoice::A, s.clone())),
            (None, Some(s)) => Some((VoteChoice::B, s.clone())),
            _ => None,
        };
        if let Some((choice, song)) = win {
            m.winner = Some(choice);
            m.state = MatchState::Done;
            m.timer.running = false;
            if m.round < tr {
                let (pr, pi, is_a) = parent(m.round, m.idx);
                advance.push((pr, pi, is_a, song));
            }
        }
    }
    for (pr, pi, is_a, song) in advance {
        if let Some(t) = matches.iter_mut().find(|m| m.round == pr && m.idx == pi) {
            if is_a {
                t.a = Some(song);
            } else {
                t.b = Some(song);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::song::Source;

    fn songs(n: usize) -> Vec<Song> {
        (0..n)
            .map(|i| Song {
                id: format!("s{i}"),
                title: format!("song {i}"),
                artist: None,
                thumbnail: None,
                duration_sec: None,
                source: Source::Youtube,
                source_url: format!("https://x/{i}"),
                submitter: None,
                metadata: None,
            })
            .collect()
    }

    #[test]
    fn total_rounds_correct() {
        assert_eq!(total_rounds(1), 0);
        assert_eq!(total_rounds(2), 1);
        assert_eq!(total_rounds(3), 2);
        assert_eq!(total_rounds(4), 2);
        assert_eq!(total_rounds(5), 3);
        assert_eq!(total_rounds(8), 3);
        assert_eq!(total_rounds(9), 4);
    }

    #[test]
    fn power_of_two_has_no_byes() {
        let m = generate(&songs(4), 30);
        // 2 first-round matches + 1 final = 3
        assert_eq!(m.len(), 3);
        let r1: Vec<_> = m.iter().filter(|x| x.round == 1).collect();
        assert_eq!(r1.len(), 2);
        assert!(r1.iter().all(|x| x.a.is_some() && x.b.is_some()));
        assert!(r1.iter().all(|x| x.state == MatchState::Pending));
    }

    #[test]
    fn non_power_of_two_resolves_byes_no_empty_matches() {
        let m = generate(&songs(5), 30);
        // slots=8 -> r1:4, r2:2, r3:1 => 7 matches
        assert_eq!(m.len(), 7);
        // round 1 has no both-None ghost (seeding invariant); later rounds may
        // legitimately start empty and fill as winners advance.
        assert!(m
            .iter()
            .filter(|x| x.round == 1)
            .all(|x| x.a.is_some() || x.b.is_some()));
        // 3 byes auto-done in round 1, leaving exactly 1 real round-1 game
        let r1_done = m
            .iter()
            .filter(|x| x.round == 1 && x.state == MatchState::Done)
            .count();
        assert_eq!(r1_done, 3);
        // their winners advanced into round 2 slots
        let r2_filled = m
            .iter()
            .filter(|x| x.round == 2)
            .map(|x| x.a.is_some() as u32 + x.b.is_some() as u32)
            .sum::<u32>();
        assert_eq!(r2_filled, 3);
    }

    #[test]
    fn parent_mapping() {
        assert_eq!(parent(1, 0), (2, 0, true));
        assert_eq!(parent(1, 1), (2, 0, false));
        assert_eq!(parent(1, 2), (2, 1, true));
        assert_eq!(parent(1, 3), (2, 1, false));
    }
}
