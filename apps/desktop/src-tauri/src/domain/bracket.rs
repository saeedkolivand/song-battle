//! Bracket generation + routing for single-elim, best-of-three (same tree,
//! `best_of=3`), and double-elim (winners / losers / grand with bracket reset).
//!
//! Each match carries routing pointers `win_to` / `lose_to`: resolution just
//! delivers the winning/losing song downstream, and `settle` auto-resolves byes.
//! Routing is a pure function of `(group, round, idx, k)` (`route_for`/`wire`), so
//! it can be recomputed after a DB load instead of being persisted.

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

/// Which sub-bracket a match belongs to. `Main` for single-elim/bo3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchGroup {
    #[default]
    Main,
    Winners,
    Losers,
    Grand,
}

/// A routing target: a specific slot of a specific match. Derived (never persisted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dest {
    pub group: MatchGroup,
    pub round: u32,
    pub idx: u32,
    pub slot_a: bool,
}

fn default_best_of() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub id: String,
    #[serde(default)]
    pub group: MatchGroup,
    pub round: u32, // 1-based within its group
    pub idx: u32,   // 0-based within (group, round)
    pub a: Option<Song>,
    pub b: Option<Song>,
    #[serde(default)]
    pub votes: Votes,
    pub state: MatchState,
    pub winner: Option<VoteChoice>,
    pub timer: Timer,
    #[serde(default = "default_best_of")]
    pub best_of: u32,
    #[serde(default)]
    pub wins_a: u32,
    #[serde(default)]
    pub wins_b: u32,
    // Derived after generation / load — not the source of truth in the DB.
    #[serde(skip)]
    pub win_to: Option<Dest>,
    #[serde(skip)]
    pub lose_to: Option<Dest>,
    #[serde(default)]
    pub a_filled: bool,
    #[serde(default)]
    pub b_filled: bool,
}

impl Match {
    #[allow(clippy::too_many_arguments)]
    fn new(
        group: MatchGroup,
        round: u32,
        idx: u32,
        a: Option<Song>,
        b: Option<Song>,
        duration_sec: u32,
        best_of: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            group,
            round,
            idx,
            a,
            b,
            votes: Votes::default(),
            state: MatchState::Pending,
            winner: None,
            timer: Timer::new(duration_sec),
            best_of,
            wins_a: 0,
            wins_b: 0,
            win_to: None,
            lose_to: None,
            a_filled: false,
            b_filled: false,
        }
    }
}

/// `ceil(log2(n))` = number of winners/main rounds; 0 for n < 2.
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

/// Single-elim parent in a binary tree.
pub fn parent(round: u32, idx: u32) -> (u32, u32, bool) {
    (round + 1, idx / 2, idx.is_multiple_of(2))
}

/// Single-elim tree (group `Main`). `best_of` is 1 for single, 3 for bo3.
pub fn generate_single(songs: &[Song], duration_sec: u32, best_of: u32) -> Vec<Match> {
    let k = total_rounds(songs.len());
    if k == 0 {
        return Vec::new();
    }
    let slots = 1usize << k;
    let mut matches = Vec::new();
    for m in 0..slots / 2 {
        let a = songs.get(m).cloned();
        let b = songs.get(slots - 1 - m).cloned();
        let mut mt = Match::new(MatchGroup::Main, 1, m as u32, a, b, duration_sec, best_of);
        mt.a_filled = true;
        mt.b_filled = true;
        matches.push(mt);
    }
    for round in 2..=k {
        for m in 0..(slots >> round) {
            matches.push(Match::new(
                MatchGroup::Main,
                round,
                m as u32,
                None,
                None,
                duration_sec,
                best_of,
            ));
        }
    }
    wire(&mut matches, k);
    matches
}

/// Double-elim: winners tree + losers bracket + grand final (with reset). Song
/// count is padded to the next power of two by byes (`best_of=1`).
pub fn generate_double(songs: &[Song], duration_sec: u32) -> Vec<Match> {
    let k = total_rounds(songs.len());
    if k == 0 {
        return Vec::new();
    }
    let p = 1usize << k;
    let mut matches = Vec::new();

    // Winners bracket (round 1 seeded; later rounds empty).
    for m in 0..p / 2 {
        let a = songs.get(m).cloned();
        let b = songs.get(p - 1 - m).cloned();
        let mut mt = Match::new(MatchGroup::Winners, 1, m as u32, a, b, duration_sec, 1);
        mt.a_filled = true;
        mt.b_filled = true;
        matches.push(mt);
    }
    for round in 2..=k {
        for m in 0..(p >> round) {
            matches.push(Match::new(
                MatchGroup::Winners,
                round,
                m as u32,
                None,
                None,
                duration_sec,
                1,
            ));
        }
    }
    // Losers bracket: 2k-2 rounds, alternating minor (internal) / major (WB drop-in).
    if k >= 2 {
        for round in 1..=(2 * k - 2) {
            for m in 0..lb_round_size(p, round) {
                matches.push(Match::new(
                    MatchGroup::Losers,
                    round,
                    m as u32,
                    None,
                    None,
                    duration_sec,
                    1,
                ));
            }
        }
    }
    // Grand final + reset decider.
    matches.push(Match::new(MatchGroup::Grand, 1, 0, None, None, duration_sec, 1));
    matches.push(Match::new(MatchGroup::Grand, 2, 0, None, None, duration_sec, 1));

    wire(&mut matches, k);
    matches
}

/// Number of matches in losers-bracket round `round` (1-based).
fn lb_round_size(p: usize, round: u32) -> usize {
    let j = round.div_ceil(2); // pair index: minor 2j-1 and major 2j share a size
    p >> (j as usize + 1)
}

/// (Re)compute `win_to`/`lose_to` for every match from its coordinates.
pub fn wire(matches: &mut [Match], k: u32) {
    for m in matches.iter_mut() {
        let (w, l) = route_for(m.group, m.round, m.idx, k);
        m.win_to = w;
        m.lose_to = l;
    }
}

fn route_for(group: MatchGroup, round: u32, idx: u32, k: u32) -> (Option<Dest>, Option<Dest>) {
    match group {
        MatchGroup::Main => {
            let win = (round < k).then(|| {
                let (r, i, a) = parent(round, idx);
                Dest {
                    group: MatchGroup::Main,
                    round: r,
                    idx: i,
                    slot_a: a,
                }
            });
            (win, None)
        }
        MatchGroup::Winners => {
            let win = if round < k {
                let (r, i, a) = parent(round, idx);
                Dest {
                    group: MatchGroup::Winners,
                    round: r,
                    idx: i,
                    slot_a: a,
                }
            } else {
                Dest {
                    group: MatchGroup::Grand,
                    round: 1,
                    idx: 0,
                    slot_a: true,
                } // winners champion → grand slot a
            };
            let lose = if round == 1 {
                if k == 1 {
                    // No losers bracket (2 songs): the single loser is the LB champ.
                    Dest {
                        group: MatchGroup::Grand,
                        round: 1,
                        idx: 0,
                        slot_a: false,
                    }
                } else {
                    // Pair adjacent WB-r1 losers into LB round 1.
                    Dest {
                        group: MatchGroup::Losers,
                        round: 1,
                        idx: idx / 2,
                        slot_a: idx.is_multiple_of(2),
                    }
                }
            } else {
                // WB-round-r loser drops into LB major round 2(r-1), slot b.
                Dest {
                    group: MatchGroup::Losers,
                    round: 2 * (round - 1),
                    idx,
                    slot_a: false,
                }
            };
            (Some(win), Some(lose))
        }
        MatchGroup::Losers => {
            let last = 2 * k - 2;
            let win = if round == last {
                Dest {
                    group: MatchGroup::Grand,
                    round: 1,
                    idx: 0,
                    slot_a: false,
                } // losers champion → grand slot b
            } else if round % 2 == 1 {
                // minor → next major (same idx, slot a)
                Dest {
                    group: MatchGroup::Losers,
                    round: round + 1,
                    idx,
                    slot_a: true,
                }
            } else {
                // major → next minor (pair, slot per parity)
                Dest {
                    group: MatchGroup::Losers,
                    round: round + 1,
                    idx: idx / 2,
                    slot_a: idx.is_multiple_of(2),
                }
            };
            (Some(win), None)
        }
        MatchGroup::Grand => (None, None), // handled specially (bracket reset)
    }
}

pub fn find_idx(matches: &[Match], d: Dest) -> Option<usize> {
    matches
        .iter()
        .position(|m| m.group == d.group && m.round == d.round && m.idx == d.idx)
}

/// Deliver a song (or a bye `None`) into a routing target, marking that slot filled.
pub fn deliver(matches: &mut [Match], dest: Option<Dest>, song: Option<Song>) {
    let Some(d) = dest else { return };
    if let Some(i) = find_idx(matches, d) {
        if d.slot_a {
            matches[i].a = song;
            matches[i].a_filled = true;
        } else {
            matches[i].b = song;
            matches[i].b_filled = true;
        }
    }
}

/// Resolve every ready bye (a fully-fed match with fewer than two real songs),
/// cascading downstream. A real two-song match is left `Pending` for live play.
pub fn settle(matches: &mut [Match]) {
    while let Some(i) = matches.iter().position(|m| {
        m.state == MatchState::Pending && m.a_filled && m.b_filled && (m.a.is_none() || m.b.is_none())
    }) {
        let (choice, win_song, lose_song) = match (matches[i].a.clone(), matches[i].b.clone()) {
            (Some(s), None) => (Some(VoteChoice::A), Some(s), None),
            (None, Some(s)) => (Some(VoteChoice::B), Some(s), None),
            (None, None) => (None, None, None),
            (Some(_), Some(_)) => break, // not a bye
        };
        matches[i].winner = choice;
        matches[i].state = MatchState::Done;
        matches[i].timer.running = false;
        let (w, l) = (matches[i].win_to, matches[i].lose_to);
        deliver(matches, w, win_song);
        deliver(matches, l, lose_song);
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

    fn count(m: &[Match], g: MatchGroup) -> usize {
        m.iter().filter(|x| x.group == g).count()
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
    fn single_power_of_two_has_no_byes() {
        let m = generate_single(&songs(4), 30, 1);
        assert_eq!(m.len(), 3); // 2 r1 + 1 final
        assert!(m.iter().all(|x| x.group == MatchGroup::Main && x.best_of == 1));
        let r1: Vec<_> = m.iter().filter(|x| x.round == 1).collect();
        assert_eq!(r1.len(), 2);
        assert!(r1.iter().all(|x| x.a.is_some() && x.b.is_some()));
    }

    #[test]
    fn bo3_is_single_structure_with_best_of_3() {
        let m = generate_single(&songs(4), 30, 3);
        assert_eq!(m.len(), 3);
        assert!(m.iter().all(|x| x.group == MatchGroup::Main && x.best_of == 3));
    }

    #[test]
    fn double_4_structure_and_routing() {
        let m = generate_double(&songs(4), 30);
        // winners 3 (2 r1 + 1 final), losers 2 (1 + final), grand 2 (final + reset)
        assert_eq!(count(&m, MatchGroup::Winners), 3);
        assert_eq!(count(&m, MatchGroup::Losers), 2);
        assert_eq!(count(&m, MatchGroup::Grand), 2);
        assert_eq!(m.len(), 7);

        let wf = |round, idx| {
            m.iter()
                .find(|x| x.group == MatchGroup::Winners && x.round == round && x.idx == idx)
                .unwrap()
        };
        // WB r1 losers pair into LB r1 idx0 (a / b).
        let l0 = wf(1, 0).lose_to.unwrap();
        assert_eq!(l0.group, MatchGroup::Losers);
        assert_eq!(l0.round, 1, "WB-r1-0 loser goes to LB round 1");
        assert_eq!(l0.idx, 0, "WB-r1-0 loser goes to LB-r1 idx 0");
        assert!(l0.slot_a, "WB-r1-0 (even idx) fills slot a");
        let l1 = wf(1, 1).lose_to.unwrap();
        assert_eq!(l1.group, MatchGroup::Losers);
        assert_eq!(l1.round, 1, "WB-r1-1 loser goes to LB round 1");
        assert_eq!(l1.idx, 0, "WB-r1-1 loser pairs with WB-r1-0 in LB-r1 idx 0");
        assert!(!l1.slot_a, "WB-r1-1 (odd idx) fills slot b");
        // WB final winner → grand a; its loser → losers final (round 2) slot b.
        assert_eq!(wf(2, 0).win_to.unwrap().group, MatchGroup::Grand);
        assert!(wf(2, 0).win_to.unwrap().slot_a);
        let wf2_lose = wf(2, 0).lose_to.unwrap();
        assert_eq!(wf2_lose.group, MatchGroup::Losers);
        assert_eq!(wf2_lose.round, 2, "WB-final loser drops to LB round 2");
        assert_eq!(wf2_lose.idx, 0, "WB-final loser drops to LB-r2 idx 0");
    }

    #[test]
    fn double_8_structure_and_loser_routing() {
        let m = generate_double(&songs(8), 30);
        assert_eq!(count(&m, MatchGroup::Winners), 7); // 4 + 2 + 1
        assert_eq!(count(&m, MatchGroup::Losers), 6); // 2 + 2 + 1 + 1
        assert_eq!(count(&m, MatchGroup::Grand), 2);
        assert_eq!(m.len(), 15);

        let lf = |round| {
            m.iter()
                .filter(|x| x.group == MatchGroup::Losers && x.round == round)
                .count()
        };
        assert_eq!((lf(1), lf(2), lf(3), lf(4)), (2, 2, 1, 1));

        // WB round-2 loser drops into LB major round 2 (slot b).
        let w2 = m
            .iter()
            .find(|x| x.group == MatchGroup::Winners && x.round == 2 && x.idx == 0)
            .unwrap();
        let d = w2.lose_to.unwrap();
        assert_eq!((d.group, d.round, d.slot_a), (MatchGroup::Losers, 2, false));
        // Losers final (round 4) winner → grand slot b.
        let lfin = m
            .iter()
            .find(|x| x.group == MatchGroup::Losers && x.round == 4)
            .unwrap();
        assert_eq!(lfin.win_to.unwrap().group, MatchGroup::Grand);
        assert!(!lfin.win_to.unwrap().slot_a);
    }

    #[test]
    fn double_6_pads_with_byes() {
        let mut m = generate_double(&songs(6), 30);
        assert_eq!(m.len(), 15); // padded to 8
        settle(&mut m);
        // 2 byes auto-resolve in winners round 1.
        let wb_r1_done = m
            .iter()
            .filter(|x| x.group == MatchGroup::Winners && x.round == 1 && x.state == MatchState::Done)
            .count();
        assert_eq!(wb_r1_done, 2);
    }

    /// With 6 songs (padded to 8), WB-r1 slots 0 and 1 are byes (songs 6 and 7
    /// absent). Their losers are both None, so LB-r1-idx0 receives (None, None)
    /// and cascades to Done as well. LB-r1-idx1 waits for real WB-r1 losers.
    #[test]
    fn double_6_lb_r1_byes_cascade() {
        let mut m = generate_double(&songs(6), 30);
        settle(&mut m);
        // Exactly one LB-r1 match auto-resolves (the one fed by the two byes).
        let lb_r1_done = m
            .iter()
            .filter(|x| {
                x.group == MatchGroup::Losers && x.round == 1 && x.state == MatchState::Done
            })
            .count();
        assert_eq!(lb_r1_done, 1, "bye cascade: LB-r1-idx0 resolves; LB-r1-idx1 waits for real losers");
    }
}
