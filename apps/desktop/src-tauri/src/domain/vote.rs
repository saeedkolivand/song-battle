use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which side a viewer voted for. Serializes `"a"` / `"b"` to match TS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoteChoice {
    A,
    B,
}

/// Parse a chat message into a vote. Accepts the six documented forms,
/// trimmed and case-insensitive; everything else is `None`.
pub fn parse_vote(text: &str) -> Option<VoteChoice> {
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "!1" | "!vote 1" => Some(VoteChoice::A),
        "2" | "!2" | "!vote 2" => Some(VoteChoice::B),
        _ => None,
    }
}

/// Integer percentage 0..=100 (floored). `total == 0` → 0.
pub fn pct(votes: u32, total: u32) -> u32 {
    (votes * 100).checked_div(total).unwrap_or(0)
}

const COOLDOWN_MS: u64 = 250;

/// One vote per user (last write wins), with a small per-user cooldown to drop
/// burst spam. `last` is runtime-only (not persisted/exported).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Votes {
    map: HashMap<String, VoteChoice>,
    #[serde(skip)]
    last: HashMap<String, u64>,
}

impl Votes {
    /// Record `choice` for `user_id`. Returns false if dropped (cooldown).
    pub fn cast(&mut self, user_id: String, choice: VoteChoice, now_ms: u64) -> bool {
        if let Some(&t) = self.last.get(&user_id) {
            if now_ms.saturating_sub(t) < COOLDOWN_MS {
                return false;
            }
        }
        self.last.insert(user_id.clone(), now_ms);
        self.map.insert(user_id, choice);
        true
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.last.clear();
    }

    /// (votes for A, votes for B).
    pub fn tally(&self) -> (u32, u32) {
        let mut a = 0;
        let mut b = 0;
        for c in self.map.values() {
            match c {
                VoteChoice::A => a += 1,
                VoteChoice::B => b += 1,
            }
        }
        (a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_six_forms() {
        for s in ["1", "!1", "!vote 1", " 1 ", "!VOTE 1"] {
            assert_eq!(parse_vote(s), Some(VoteChoice::A), "{s:?}");
        }
        for s in ["2", "!2", "!vote 2"] {
            assert_eq!(parse_vote(s), Some(VoteChoice::B), "{s:?}");
        }
    }

    #[test]
    fn rejects_junk() {
        for s in ["", "3", "vote", "1!", "a", "11", "!vote", "hello 1"] {
            assert_eq!(parse_vote(s), None, "{s:?}");
        }
    }

    #[test]
    fn one_vote_per_user_change_overwrites() {
        let mut v = Votes::default();
        assert!(v.cast("u1".into(), VoteChoice::A, 0));
        assert!(v.cast("u2".into(), VoteChoice::A, 0));
        // same user changes mind later -> overwrite, not a second vote
        assert!(v.cast("u1".into(), VoteChoice::B, 1000));
        assert_eq!(v.tally(), (1, 1));
    }

    #[test]
    fn cooldown_drops_burst() {
        let mut v = Votes::default();
        assert!(v.cast("u1".into(), VoteChoice::A, 0));
        assert!(!v.cast("u1".into(), VoteChoice::B, 100)); // within cooldown
        assert_eq!(v.tally(), (1, 0));
    }

    #[test]
    fn pct_math() {
        assert_eq!(pct(0, 0), 0);
        assert_eq!(pct(1, 2), 50);
        assert_eq!(pct(2, 3), 66); // floored
        assert_eq!(pct(1, 3), 33);
        assert_eq!(pct(3, 3), 100);
    }
}
