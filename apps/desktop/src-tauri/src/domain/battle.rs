//! The battle aggregate: songs, the bracket, the current-match pointer, and the
//! rules that drive voting → winner → advancement. Pure (no IO, no wall clock —
//! `tick()` is one logical second, `now_ms` is injected).

use crate::domain::{
    bracket::{self, Match, MatchState},
    song::Song,
    vote::VoteChoice,
};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_DURATION: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BattleStatus {
    Idle,
    Running,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Battle {
    pub id: String,
    pub title: String,
    pub description: String,
    pub theme: String,
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

    pub fn generate_bracket(&mut self) -> AppResult<()> {
        if self.songs.len() < 2 {
            return Err(AppError::Invalid("need at least 2 songs".into()));
        }
        self.matches = bracket::generate(&self.songs, self.duration_sec);
        self.total_rounds = bracket::total_rounds(self.songs.len());
        self.current = self.first_playable();
        self.winner = None;
        self.status = BattleStatus::Idle;
        Ok(())
    }

    /// Lowest-ordered pending match whose two slots are both decided.
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
    /// `current` (malformed import / DB row) becomes `None` rather than
    /// panicking later inside a write guard. Called on the import/load boundaries.
    pub fn normalize(&mut self) {
        if self.current.is_some_and(|i| i >= self.matches.len()) {
            self.current = None;
        }
    }

    /// Force the current match to resolve now (operator skip).
    pub fn skip_match(&mut self) -> AppResult<()> {
        let i = self
            .current
            .ok_or_else(|| AppError::Invalid("no current match".into()))?;
        self.resolve(i);
        Ok(())
    }

    /// One logical second. Returns `(redraw, resolved)`: `redraw` while a match
    /// is active (countdown changed), `resolved` when one just completed.
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
            self.resolve(i);
            (true, true)
        } else {
            (true, false)
        }
    }

    /// Close voting on match `i`, pick the winner (ties → A), advance the song,
    /// and move on — finishing the battle if this was the final. A bad `i`
    /// (out-of-range) is a safe no-op.
    fn resolve(&mut self, i: usize) {
        let Some(m) = self.matches.get(i) else { return };
        let (va, vb) = m.votes.tally();
        let choice = if vb > va {
            VoteChoice::B
        } else {
            VoteChoice::A
        };
        let round = m.round;
        let idx = m.idx;
        let win_song = match choice {
            VoteChoice::A => m.a.clone(),
            VoteChoice::B => m.b.clone(),
        };

        if let Some(m) = self.matches.get_mut(i) {
            m.winner = Some(choice);
            m.state = MatchState::Done;
            m.timer.running = false;
            m.timer.remaining_sec = 0;
        }

        if round >= self.total_rounds {
            self.winner = win_song;
            self.status = BattleStatus::Finished;
            self.current = None;
            return;
        }
        if let Some(song) = win_song {
            let (pr, pi, is_a) = bracket::parent(round, idx);
            if let Some(t) = self.matches.iter_mut().find(|m| m.round == pr && m.idx == pi) {
                if is_a {
                    t.a = Some(song);
                } else {
                    t.b = Some(song);
                }
            }
        }
        self.current = self.first_playable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::song::Source;

    fn battle_with(n: usize) -> Battle {
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
        b.generate_bracket().unwrap();
        b
    }

    fn run_match_to_winner(b: &mut Battle, votes_a: usize, votes_b: usize) {
        b.start_match().unwrap();
        let mut now = 0u64;
        for k in 0..votes_a {
            now += 1000;
            assert!(b.cast_vote(format!("a{k}"), VoteChoice::A, now));
        }
        for k in 0..votes_b {
            now += 1000;
            assert!(b.cast_vote(format!("b{k}"), VoteChoice::B, now));
        }
        // tick the 2s timer to expiry
        b.tick();
        b.tick();
    }

    #[test]
    fn expiry_picks_higher_votes_and_advances() {
        let mut b = battle_with(2); // single final match
        let a_id = b.matches[0].a.clone().unwrap().id;
        run_match_to_winner(&mut b, 3, 1);
        assert_eq!(b.matches[0].state, MatchState::Done);
        assert_eq!(b.matches[0].winner, Some(VoteChoice::A));
        assert_eq!(b.status, BattleStatus::Finished);
        assert_eq!(b.winner.as_ref().unwrap().id, a_id);
    }

    #[test]
    fn tie_goes_to_a() {
        let mut b = battle_with(2);
        run_match_to_winner(&mut b, 2, 2);
        assert_eq!(b.matches[0].winner, Some(VoteChoice::A));
    }

    #[test]
    fn full_four_song_bracket_to_final() {
        let mut b = battle_with(4);
        assert_eq!(b.total_rounds, 2);
        // round 1, match 0: B wins
        run_match_to_winner(&mut b, 0, 1);
        // round 1, match 1: A wins
        run_match_to_winner(&mut b, 1, 0);
        // both winners should now sit in the final (round 2)
        let final_idx = b.matches.iter().position(|m| m.round == 2).unwrap();
        assert!(b.matches[final_idx].a.is_some() && b.matches[final_idx].b.is_some());
        assert_eq!(b.status, BattleStatus::Running);
        // play the final
        run_match_to_winner(&mut b, 5, 0);
        assert_eq!(b.status, BattleStatus::Finished);
        assert!(b.winner.is_some());
        assert!(b.matches.iter().all(|m| m.state == MatchState::Done));
    }

    #[test]
    fn skip_resolves_immediately() {
        let mut b = battle_with(2);
        b.start_match().unwrap();
        b.skip_match().unwrap();
        assert_eq!(b.status, BattleStatus::Finished);
    }

    #[test]
    fn generate_bracket_needs_two_songs() {
        let mut b = Battle::new("t".into(), "d".into(), "th".into());
        assert!(b.generate_bracket().is_err());
    }

    #[test]
    fn out_of_range_current_is_normalized_and_never_panics() {
        // Simulates a malformed import/DB row: current points past matches.
        let mut b = battle_with(2); // 1 match, current = Some(0)
        b.current = Some(99);

        // Defensive ops must be no-ops, not index panics, even before normalize.
        assert_eq!(b.tick(), (false, false));
        assert!(!b.cast_vote("u".into(), VoteChoice::A, 0));
        b.reset_votes();
        assert!(b.skip_match().is_ok()); // resolve(99) safely no-ops

        // normalize clamps the bad pointer to None.
        b.normalize();
        assert_eq!(b.current, None);
    }
}
