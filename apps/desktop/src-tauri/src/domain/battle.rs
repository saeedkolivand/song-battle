//! The battle aggregate: songs, the bracket, the current-match pointer, and the
//! game → series → match rules. Pure (no IO, no wall clock — `tick()` is one
//! logical second, `now_ms` is injected).
//!
//! A "game" is one timed vote round. A match is a best-of-`best_of` series of
//! games; deciding a match advances the winner (and, for double-elim, drops the
//! loser) via the bracket's routing pointers. `round` is the 1-based index within
//! a match's group; `total_rounds` is the winners/main round count.

use crate::domain::{
    bracket::{self, Match, MatchGroup, MatchState},
    song::Song,
    vote::VoteChoice,
};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_DURATION: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BattleStatus {
    #[default]
    Idle,
    Running,
    Finished,
}

/// Tournament structure. `Single`/`Double` collapse the series to one game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BattleMode {
    #[default]
    Single,
    Double,
    Bo3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Battle {
    pub id: String,
    pub title: String,
    pub description: String,
    pub theme: String,
    #[serde(default)]
    pub mode: BattleMode,
    pub status: BattleStatus,
    pub songs: Vec<Song>,
    pub matches: Vec<Match>,
    pub total_rounds: u32,
    pub current: Option<usize>,
    pub winner: Option<Song>,
    pub duration_sec: u32,
}

impl Battle {
    pub fn new(title: String, description: String, theme: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            description,
            theme,
            mode: BattleMode::Single,
            status: BattleStatus::Idle,
            songs: Vec::new(),
            matches: Vec::new(),
            total_rounds: 0,
            current: None,
            winner: None,
            duration_sec: DEFAULT_DURATION,
        }
    }

    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
    }

    pub fn remove_song(&mut self, id: &str) {
        self.songs.retain(|s| s.id != id);
    }

    pub fn shuffle<R: rand::Rng>(&mut self, rng: &mut R) {
        use rand::seq::SliceRandom;
        self.songs.shuffle(rng);
    }

    pub fn set_timer(&mut self, duration_sec: u32) {
        self.duration_sec = duration_sec;
        for m in &mut self.matches {
            m.timer.duration_sec = duration_sec;
            if !m.timer.running {
                m.timer.remaining_sec = duration_sec;
            }
        }
    }

    pub fn generate_bracket(&mut self, mode: BattleMode) -> AppResult<()> {
        if self.songs.len() < 2 {
            return Err(AppError::Invalid("need at least 2 songs".into()));
        }
        self.mode = mode;
        self.matches = match mode {
            BattleMode::Single => bracket::generate_single(&self.songs, self.duration_sec, 1),
            BattleMode::Bo3 => bracket::generate_single(&self.songs, self.duration_sec, 3),
            BattleMode::Double => bracket::generate_double(&self.songs, self.duration_sec),
        };
        self.total_rounds = bracket::total_rounds(self.songs.len());
        bracket::settle(&mut self.matches); // auto-resolve byes
        self.current = self.first_playable();
        self.winner = None;
        self.status = BattleStatus::Idle;
        Ok(())
    }

    /// Recompute derived routing + fill flags after a DB load / JSON import.
    pub fn rewire(&mut self) {
        bracket::wire(&mut self.matches, self.total_rounds);
        self.recompute_filled();
    }

    fn recompute_filled(&mut self) {
        for m in &mut self.matches {
            m.a_filled = m.a.is_some();
            m.b_filled = m.b.is_some();
            // Round-1 winners/main slots are seeded (a settled bye counts as filled).
            if matches!(m.group, MatchGroup::Main | MatchGroup::Winners) && m.round == 1 {
                m.a_filled = true;
                m.b_filled = true;
            }
        }
        // A resolved feeder has delivered to its targets (incl. settled-bye `None`s).
        let delivered: Vec<bracket::Dest> = self
            .matches
            .iter()
            .filter(|m| m.state == MatchState::Done)
            .flat_map(|m| [m.win_to, m.lose_to])
            .flatten()
            .collect();
        for d in delivered {
            if let Some(i) = bracket::find_idx(&self.matches, d) {
                if d.slot_a {
                    self.matches[i].a_filled = true;
                } else {
                    self.matches[i].b_filled = true;
                }
            }
        }
    }

    /// Lowest-ordered pending match with both slots holding a real song.
    fn first_playable(&self) -> Option<usize> {
        self.matches
            .iter()
            .position(|m| m.state == MatchState::Pending && m.a.is_some() && m.b.is_some())
    }

    pub fn start_match(&mut self) -> AppResult<()> {
        let i = self
            .first_playable()
            .ok_or_else(|| AppError::Invalid("no pending match to start".into()))?;
        self.current = Some(i);
        let m = &mut self.matches[i];
        m.votes.clear();
        m.state = MatchState::Active;
        m.timer.start();
        self.status = BattleStatus::Running;
        Ok(())
    }

    pub fn reset_votes(&mut self) {
        let Some(i) = self.current else { return };
        if let Some(m) = self.matches.get_mut(i) {
            m.votes.clear();
            if m.state == MatchState::Active {
                m.timer.start();
            }
        }
    }

    pub fn cast_vote(&mut self, user_id: String, choice: VoteChoice, now_ms: u64) -> bool {
        let Some(i) = self.current else { return false };
        match self.matches.get_mut(i) {
            Some(m) if m.state == MatchState::Active && m.timer.running => {
                m.votes.cast(user_id, choice, now_ms)
            }
            _ => false,
        }
    }

    /// Clamp untrusted/loaded state into a bootable shape: an out-of-range
    /// `current` becomes `None` rather than panicking later inside a write guard.
    pub fn normalize(&mut self) {
        if self.current.is_some_and(|i| i >= self.matches.len()) {
            self.current = None;
        }
    }

    /// Force the current match to resolve now (operator skip). Picks the series
    /// leader, else the current game's tally (tie → A).
    pub fn skip_match(&mut self) -> AppResult<()> {
        let i = self
            .current
            .ok_or_else(|| AppError::Invalid("no current match".into()))?;
        let Some(m) = self.matches.get(i) else {
            return Ok(());
        };
        let choice = if m.wins_b > m.wins_a {
            VoteChoice::B
        } else if m.wins_a > m.wins_b {
            VoteChoice::A
        } else {
            let (va, vb) = m.votes.tally();
            if vb > va {
                VoteChoice::B
            } else {
                VoteChoice::A
            }
        };
        self.decide_match(i, choice);
        Ok(())
    }

    /// One logical second. `(redraw, persist)`: `redraw` while a match is active,
    /// `persist` when a game completed (wins/bracket changed).
    pub fn tick(&mut self) -> (bool, bool) {
        let Some(i) = self.current else {
            return (false, false);
        };
        let Some(m) = self.matches.get_mut(i) else {
            return (false, false);
        };
        if m.state != MatchState::Active {
            return (false, false);
        }
        if m.timer.tick() {
            self.resolve_game(i);
            (true, true)
        } else {
            (true, false)
        }
    }

    /// A game's timer expired: score it; decide the match if a side clinched the
    /// series, else auto-continue with a fresh game (same two songs).
    fn resolve_game(&mut self, i: usize) {
        let (va, vb) = self.matches[i].votes.tally();
        let choice = if vb > va {
            VoteChoice::B
        } else {
            VoteChoice::A
        };
        let (best_of, wins_now) = {
            let m = &mut self.matches[i];
            match choice {
                VoteChoice::A => m.wins_a += 1,
                VoteChoice::B => m.wins_b += 1,
            }
            let w = if choice == VoteChoice::A {
                m.wins_a
            } else {
                m.wins_b
            };
            (m.best_of, w)
        };
        if wins_now > best_of / 2 {
            self.decide_match(i, choice);
        } else {
            // Series continues: hands-off restart of the countdown for the next game.
            let m = &mut self.matches[i];
            m.votes.clear();
            m.timer.start();
        }
    }

    /// Conclude match `i` for `choice`: set the winner, then advance/drop songs
    /// (handling the double-elim grand final + bracket reset) and pick the next
    /// match, or finish the battle.
    fn decide_match(&mut self, i: usize, choice: VoteChoice) {
        let (group, round, win_to, lose_to, a, b) = {
            let m = &self.matches[i];
            (m.group, m.round, m.win_to, m.lose_to, m.a.clone(), m.b.clone())
        };
        let (win_song, lose_song) = match choice {
            VoteChoice::A => (a.clone(), b.clone()),
            VoteChoice::B => (b.clone(), a.clone()),
        };
        {
            let m = &mut self.matches[i];
            m.winner = Some(choice);
            m.state = MatchState::Done;
            m.timer.running = false;
            m.timer.remaining_sec = 0;
        }

        if group == MatchGroup::Grand {
            self.resolve_grand(round, choice, win_song, a, b);
            return;
        }

        bracket::deliver(&mut self.matches, win_to, win_song.clone());
        bracket::deliver(&mut self.matches, lose_to, lose_song);
        bracket::settle(&mut self.matches);

        if win_to.is_none() {
            // Terminal main match (single / bo3 final).
            self.winner = win_song;
            self.status = BattleStatus::Finished;
            self.current = None;
        } else {
            self.current = self.first_playable();
        }
    }

    fn resolve_grand(
        &mut self,
        round: u32,
        choice: VoteChoice,
        win_song: Option<Song>,
        a: Option<Song>,
        b: Option<Song>,
    ) {
        // Grand final (round 1): WB champion (a) winning ends it; LB champion (b)
        // winning forces the reset decider (round 2). Round 2 always ends it.
        if round == 1 && choice == VoteChoice::B {
            if let Some(j) = self
                .matches
                .iter()
                .position(|m| m.group == MatchGroup::Grand && m.round == 2)
            {
                self.matches[j].a = a;
                self.matches[j].b = b;
                self.matches[j].a_filled = true;
                self.matches[j].b_filled = true;
                self.current = Some(j);
                return;
            }
        }
        // WB champion won outright (round 1, choice A) → the unused reset is moot.
        if round == 1 {
            if let Some(j) = self
                .matches
                .iter()
                .position(|m| m.group == MatchGroup::Grand && m.round == 2)
            {
                self.matches[j].state = MatchState::Done;
            }
        }
        self.winner = win_song;
        self.status = BattleStatus::Finished;
        self.current = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::song::Source;

    fn battle(n: usize, mode: BattleMode) -> Battle {
        let mut b = Battle::new("t".into(), "d".into(), "th".into());
        for i in 0..n {
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
        b.generate_bracket(mode).unwrap();
        b
    }

    /// Cast `count` votes for `choice` (unique users, monotonic time), return next `now`.
    fn votes(b: &mut Battle, choice: VoteChoice, count: usize, mut now: u64) -> u64 {
        for k in 0..count {
            now += 1000;
            assert!(b.cast_vote(format!("u{now}_{k}"), choice, now));
        }
        now
    }

    fn expire(b: &mut Battle) {
        b.tick();
        b.tick(); // 2s timer
    }

    /// Start the current match and play one match to a winner (best-of-1).
    fn play_match(b: &mut Battle, choice: VoteChoice, now: u64) -> u64 {
        b.start_match().unwrap();
        let now = votes(b, choice, 1, now);
        expire(b);
        now
    }

    /// Drive a (best-of-1) tournament to completion, slot `a` winning every match
    /// except matches matching `pick_b` (which the `b` side wins).
    fn run_all(b: &mut Battle, pick_b: impl Fn(&Match) -> bool) -> Option<Song> {
        let mut now = 0;
        let mut guard = 0;
        while b.status != BattleStatus::Finished {
            guard += 1;
            assert!(guard < 100, "tournament did not terminate");
            let Some(i) = b.current else { break };
            let choice = if pick_b(&b.matches[i]) {
                VoteChoice::B
            } else {
                VoteChoice::A
            };
            now = play_match(b, choice, now);
        }
        b.winner.clone()
    }

    // ── single-elim regression ────────────────────────────────────────────────
    #[test]
    fn single_expiry_picks_higher_votes_and_advances() {
        let mut b = battle(2, BattleMode::Single);
        let a_id = b.matches[0].a.clone().unwrap().id;
        play_match(&mut b, VoteChoice::A, 0);
        assert_eq!(b.matches[0].winner, Some(VoteChoice::A));
        assert_eq!(b.status, BattleStatus::Finished);
        assert_eq!(b.winner.as_ref().unwrap().id, a_id);
    }

    #[test]
    fn single_tie_goes_to_a() {
        let mut b = battle(2, BattleMode::Single);
        b.start_match().unwrap();
        let now = votes(&mut b, VoteChoice::A, 2, 0);
        votes(&mut b, VoteChoice::B, 2, now);
        expire(&mut b);
        assert_eq!(b.matches[0].winner, Some(VoteChoice::A));
    }

    #[test]
    fn single_four_song_to_final() {
        let mut b = battle(4, BattleMode::Single);
        assert_eq!(b.total_rounds, 2);
        assert_eq!(b.matches.len(), 3);
        let champ = run_all(&mut b, |_| false);
        assert_eq!(b.status, BattleStatus::Finished);
        assert!(champ.is_some());
        assert!(b.matches.iter().all(|m| m.state == MatchState::Done));
    }

    #[test]
    fn single_byes_for_five() {
        let mut b = battle(5, BattleMode::Single);
        // slots=8 -> 7 matches; 3 first-round byes auto-resolved.
        assert_eq!(b.matches.len(), 7);
        let done = b
            .matches
            .iter()
            .filter(|m| m.round == 1 && m.state == MatchState::Done)
            .count();
        assert_eq!(done, 3);
        run_all(&mut b, |_| false);
        assert_eq!(b.status, BattleStatus::Finished);
    }

    #[test]
    fn skip_resolves_immediately() {
        let mut b = battle(2, BattleMode::Single);
        b.start_match().unwrap();
        b.skip_match().unwrap();
        assert_eq!(b.status, BattleStatus::Finished);
    }

    #[test]
    fn generate_bracket_needs_two_songs() {
        let mut b = Battle::new("t".into(), "d".into(), "th".into());
        assert!(b.generate_bracket(BattleMode::Single).is_err());
    }

    #[test]
    fn out_of_range_current_never_panics_and_normalizes() {
        let mut b = battle(2, BattleMode::Single);
        b.current = Some(99);
        assert_eq!(b.tick(), (false, false));
        assert!(!b.cast_vote("u".into(), VoteChoice::A, 0));
        b.reset_votes();
        assert!(b.skip_match().is_ok());
        b.normalize();
        assert_eq!(b.current, None);
    }

    // ── best-of-three ─────────────────────────────────────────────────────────
    #[test]
    fn bo3_single_game_does_not_decide() {
        let mut b = battle(2, BattleMode::Bo3);
        assert_eq!(b.matches[0].best_of, 3);
        b.start_match().unwrap();
        votes(&mut b, VoteChoice::A, 1, 0);
        expire(&mut b); // game 1 → A
        let m = &b.matches[0];
        assert_eq!(m.wins_a, 1);
        assert_eq!(m.winner, None); // series not decided
        assert_eq!(m.state, MatchState::Active); // auto-continued
        assert!(m.timer.running);
        assert_ne!(b.status, BattleStatus::Finished);
    }

    #[test]
    fn bo3_two_nil_resolves_and_advances() {
        let mut b = battle(2, BattleMode::Bo3);
        let win_id = b.matches[0].a.clone().unwrap().id;
        b.start_match().unwrap();
        let now = votes(&mut b, VoteChoice::A, 1, 0);
        expire(&mut b); // 1-0
        votes(&mut b, VoteChoice::A, 1, now);
        expire(&mut b); // 2-0 → decided
        assert_eq!(b.matches[0].wins_a, 2);
        assert_eq!(b.matches[0].winner, Some(VoteChoice::A));
        assert_eq!(b.status, BattleStatus::Finished);
        assert_eq!(b.winner.as_ref().unwrap().id, win_id);
    }

    #[test]
    fn bo3_two_one_resolves() {
        let mut b = battle(2, BattleMode::Bo3);
        b.start_match().unwrap();
        let mut now = votes(&mut b, VoteChoice::A, 1, 0);
        expire(&mut b); // 1-0 A
        now = votes(&mut b, VoteChoice::B, 1, now);
        expire(&mut b); // 1-1
        assert_eq!(b.matches[0].winner, None); // still going
        votes(&mut b, VoteChoice::A, 1, now);
        expire(&mut b); // 2-1 A
        assert_eq!((b.matches[0].wins_a, b.matches[0].wins_b), (2, 1));
        assert_eq!(b.matches[0].winner, Some(VoteChoice::A));
        assert_eq!(b.status, BattleStatus::Finished);
    }

    #[test]
    fn bo3_skip_short_circuits() {
        let mut b = battle(2, BattleMode::Bo3);
        b.start_match().unwrap();
        votes(&mut b, VoteChoice::A, 1, 0);
        expire(&mut b); // 1-0, not decided
        assert_eq!(b.status, BattleStatus::Running);
        b.skip_match().unwrap(); // leader A wins the whole match
        assert_eq!(b.matches[0].winner, Some(VoteChoice::A));
        assert_eq!(b.status, BattleStatus::Finished);
    }

    #[test]
    fn bo3_four_song_full_run() {
        let mut b = battle(4, BattleMode::Bo3);
        // 4 songs → 2 winners rounds.
        assert_eq!(b.total_rounds, 2);
        let mut now = 0;
        let mut guard = 0;
        while b.status != BattleStatus::Finished {
            guard += 1;
            assert!(guard < 100);
            b.start_match().unwrap();
            // win 2-0 for A each match
            now = votes(&mut b, VoteChoice::A, 1, now);
            expire(&mut b);
            // Guard against current=None (battle just finished in game 1, which
            // is impossible with best_of=3 since 1 > 3/2=1 is false, but keeps
            // the test panic-free if the logic ever changes).
            if b.current.is_some_and(|i| b.matches[i].state == MatchState::Active) {
                now = votes(&mut b, VoteChoice::A, 1, now);
                expire(&mut b);
            }
        }
        // Slot-a (s0) wins every match → s0 is champion.
        assert_eq!(b.winner.as_ref().unwrap().id, "s0");
        // Every match in the bracket must be resolved.
        assert!(b.matches.iter().all(|m| m.state == MatchState::Done));
    }

    // ── double-elim ───────────────────────────────────────────────────────────
    #[test]
    fn double_4_winners_path_champion() {
        let mut b = battle(4, BattleMode::Double);
        let champ = run_all(&mut b, |_| false); // a wins everything incl. GF
        assert_eq!(b.status, BattleStatus::Finished);
        // Top seed s0 wins out via the winners bracket.
        assert_eq!(champ.unwrap().id, "s0");
    }

    #[test]
    fn double_4_bracket_reset_lb_champ_wins() {
        let mut b = battle(4, BattleMode::Double);
        // b-side wins ONLY in the grand bracket → LB champ forces + wins the reset.
        let champ = run_all(&mut b, |m| m.group == MatchGroup::Grand);
        assert_eq!(b.status, BattleStatus::Finished);
        // Two grand matches were played (final + reset decider).
        let grand_done = b
            .matches
            .iter()
            .filter(|m| m.group == MatchGroup::Grand && m.state == MatchState::Done)
            .count();
        assert_eq!(grand_done, 2);
        // The losers champion (s3) won it all through the reset.
        assert_eq!(champ.unwrap().id, "s3");
    }

    #[test]
    fn double_8_completes_with_full_routing() {
        let mut b = battle(8, BattleMode::Double);
        assert_eq!(b.matches.len(), 15);
        let champ = run_all(&mut b, |_| false);
        assert_eq!(b.status, BattleStatus::Finished);
        assert_eq!(champ.unwrap().id, "s0");
        // every match except the unused reset is resolved
        let done = b
            .matches
            .iter()
            .filter(|m| m.state == MatchState::Done)
            .count();
        // With A winning everything WB-champ wins the Grand final outright; resolve_grand
        // marks the unused bracket-reset Done too → all 15 matches resolved.
        assert_eq!(done, 15);
    }

    // ── rewire ────────────────────────────────────────────────────────────────
    /// Zero all derived state (win_to / lose_to / a_filled / b_filled) on a
    /// mid-run double-elim battle, call rewire(), and verify first_playable +
    /// advancement are identical to the original.
    #[test]
    fn rewire_restores_state_after_zeroing() {
        let mut b = battle(4, BattleMode::Double);

        // Advance through both WB-r1 matches (A wins each).
        // After match 0: s0 → WB-r2-slot-a, s3 → LB-r1-slot-a.
        // After match 1: s1 → WB-r2-slot-b, s2 → LB-r1-slot-b.
        // first_playable = Some(2) = WB-r2 (index 2 in the vector).
        let mut now = play_match(&mut b, VoteChoice::A, 0);
        now = play_match(&mut b, VoteChoice::A, now);
        assert_eq!(b.current, Some(2));
        let expected_id = b.matches[2].id.clone();

        // Clone and wipe every derived field.
        let mut clone = b.clone();
        for m in clone.matches.iter_mut() {
            m.win_to = None;
            m.lose_to = None;
            m.a_filled = false;
            m.b_filled = false;
        }

        // Rewire must restore routing + fill flags.
        clone.rewire();

        // start_match uses first_playable() internally; it must find WB-r2 (index 2).
        clone.start_match().unwrap();
        assert_eq!(
            clone.matches[clone.current.unwrap()].id,
            expected_id,
            "rewire restores first_playable to the WB-r2 match"
        );
        assert_eq!(clone.matches[clone.current.unwrap()].state, MatchState::Active);

        // Verify delivery still works after rewire: play WB-r2 (A=s0 wins).
        // s0 → Grand slot-a, s1 → LB-r2-slot-b.  LB-r1 (index 3) has both songs
        // (s3, s2) → becomes first_playable.
        // Use votes+expire directly — start_match already activated WB-r2 above.
        votes(&mut clone, VoteChoice::A, 1, now);
        expire(&mut clone);
        assert_eq!(clone.current, Some(3), "after WB-r2 resolves, LB-r1 is next");
        assert_eq!(clone.matches[3].group, MatchGroup::Losers);
        assert_eq!(clone.matches[3].round, 1);
    }

    // ── grand final ───────────────────────────────────────────────────────────
    /// When the WB champion wins the Grand final outright (round 1, choice A):
    /// • the unused bracket-reset match (Grand r2) is marked Done, not played.
    /// • exactly one Grand match was actually decided.
    #[test]
    fn double_4_winners_path_grand_r2_skipped() {
        let mut b = battle(4, BattleMode::Double);
        run_all(&mut b, |_| false); // A wins every match → WB champ wins GF outright
        assert_eq!(b.status, BattleStatus::Finished);

        let grand_r2 = b
            .matches
            .iter()
            .find(|m| m.group == MatchGroup::Grand && m.round == 2)
            .expect("Grand r2 must exist");
        assert_eq!(grand_r2.state, MatchState::Done, "unused reset is marked Done");
        assert!(grand_r2.winner.is_none(), "reset match has no winner — not played");

        // Exactly one Grand match was actively decided (Grand r1 only).
        let grand_decided = b
            .matches
            .iter()
            .filter(|m| m.group == MatchGroup::Grand && m.winner.is_some())
            .count();
        assert_eq!(grand_decided, 1, "WB champ wins in a single Grand match");
    }

    // ── vote timing ───────────────────────────────────────────────────────────
    /// After the match timer expires (and the match resolves), cast_vote must
    /// return false — there is no longer an Active match accepting votes.
    #[test]
    fn vote_after_expiry_returns_false() {
        let mut b = battle(2, BattleMode::Single);
        b.start_match().unwrap();
        // Vote is accepted while the match is Active.
        assert!(b.cast_vote("u1".into(), VoteChoice::A, 1_000));
        // Let the 2s timer expire → match resolves, battle finishes.
        expire(&mut b);
        assert_eq!(b.status, BattleStatus::Finished);
        // No active match; cast_vote must return false.
        assert!(!b.cast_vote("u2".into(), VoteChoice::A, 2_000));
    }

    #[test]
    fn double_6_byes_complete() {
        let mut b = battle(6, BattleMode::Double);
        assert_eq!(b.matches.len(), 15); // padded to 8
        let byes = b
            .matches
            .iter()
            .filter(|m| {
                m.group == MatchGroup::Winners && m.round == 1 && m.state == MatchState::Done
            })
            .count();
        assert_eq!(byes, 2); // two seeds got a bye
        let champ = run_all(&mut b, |_| false);
        assert_eq!(b.status, BattleStatus::Finished);
        assert!(champ.is_some());
    }
}
