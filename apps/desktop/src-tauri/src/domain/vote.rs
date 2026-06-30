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

/// What a chat message resolves to. `Submit` (`!submit`/`!add <url>`) is open to
/// anyone; the mod-only control commands (`!reset`/`!resetvotes`, `!skip`) take
/// precedence over voting. Everything else is a vote or `Ignore`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatAction {
    Vote(VoteChoice),
    Submit(String),
    ResetVotes,
    Skip,
    Ignore,
}

pub fn classify_chat(is_mod: bool, text: &str) -> ChatAction {
    let trimmed = text.trim();
    if let Some(url) = parse_submit(trimmed) {
        return ChatAction::Submit(url); // anyone may submit
    }
    if is_mod {
        match trimmed.to_ascii_lowercase().as_str() {
            "!reset" | "!resetvotes" => return ChatAction::ResetVotes,
            "!skip" => return ChatAction::Skip,
            _ => {}
        }
    }
    match parse_vote(text) {
        Some(c) => ChatAction::Vote(c),
        None => ChatAction::Ignore,
    }
}

/// `!submit <url>` / `!add <url>` → the raw URL (first token after the command).
/// Command match is case-insensitive; the URL's case is preserved.
fn parse_submit(trimmed: &str) -> Option<String> {
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let cmd = parts.next()?;
    if !cmd.eq_ignore_ascii_case("!submit") && !cmd.eq_ignore_ascii_case("!add") {
        return None;
    }
    let url = parts.next()?.split_whitespace().next()?;
    Some(url.to_string())
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

    #[test]
    fn classify_chat_mod_commands_and_votes() {
        // mod control commands (trim + case-insensitive)
        assert_eq!(classify_chat(true, "!reset"), ChatAction::ResetVotes);
        assert_eq!(classify_chat(true, "!resetvotes"), ChatAction::ResetVotes);
        assert_eq!(classify_chat(true, " !SKIP "), ChatAction::Skip);
        // non-mods can't trigger control commands → ignored, not votes
        assert_eq!(classify_chat(false, "!reset"), ChatAction::Ignore);
        assert_eq!(classify_chat(false, "!skip"), ChatAction::Ignore);
        // everyone (mod or not) can vote
        assert_eq!(classify_chat(false, "1"), ChatAction::Vote(VoteChoice::A));
        assert_eq!(classify_chat(true, "2"), ChatAction::Vote(VoteChoice::B));
        // junk is ignored
        assert_eq!(classify_chat(true, "hello"), ChatAction::Ignore);
    }

    #[test]
    fn classify_chat_submit_parsing() {
        let u = "https://youtu.be/AbC";
        // anyone may submit; both verbs; URL case preserved
        assert_eq!(
            classify_chat(false, &format!("!submit {u}")),
            ChatAction::Submit(u.into())
        );
        assert_eq!(
            classify_chat(true, &format!("!ADD {u}")),
            ChatAction::Submit(u.into())
        );
        // surrounding whitespace + extra tokens after the URL
        assert_eq!(
            classify_chat(false, &format!("  !submit   {u}  trailing")),
            ChatAction::Submit(u.into())
        );
        // not the command, missing url → not a submit
        assert_eq!(classify_chat(false, "!submitx https://x"), ChatAction::Ignore);
        assert_eq!(classify_chat(false, "!submit"), ChatAction::Ignore);
        assert_eq!(classify_chat(false, "!add"), ChatAction::Ignore);
    }
}
