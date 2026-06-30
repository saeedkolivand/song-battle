//! Anti-flood gate for chat song submissions (`!submit <url>`). Anyone can
//! submit, so this is load-bearing: per-user cooldown + cap, a global song cap,
//! https/known-host validation, and URL dedup. The ledger is RAM-only (cleared on
//! connect / disconnect / new bracket) and the gate is fully synchronous so it
//! can run under the state lock, closing the TOCTOU before the async oEmbed fetch.

use crate::domain::{
    battle::Battle,
    song::{detect_source, Source},
};
use std::collections::HashMap;

pub const SUBMIT_COOLDOWN_MS: u64 = 15_000;
pub const PER_USER_CAP: u32 = 5;
pub const GLOBAL_SONG_CAP: usize = 64;

#[derive(Debug, Default, Clone, Copy)]
struct UserSubs {
    count: u32,
    last_ms: u64,
}

/// Per-user submission accounting (by Kick `userId`). Not persisted.
#[derive(Debug, Default)]
pub struct SubmitLedger {
    per_user: HashMap<String, UserSubs>,
}

impl SubmitLedger {
    pub fn clear(&mut self) {
        self.per_user.clear();
    }

    /// Admit one submission for `user_id` (cooldown + cap), bumping their tally.
    /// Returns false (and does NOT bump) when throttled.
    pub fn try_admit(&mut self, user_id: &str, now_ms: u64) -> bool {
        let e = self.per_user.entry(user_id.to_string()).or_default();
        if e.count >= PER_USER_CAP {
            return false;
        }
        if e.count > 0 && now_ms.saturating_sub(e.last_ms) < SUBMIT_COOLDOWN_MS {
            return false;
        }
        e.count += 1;
        e.last_ms = now_ms;
        true
    }
}

/// Normalize a URL for dedup: lowercase host, drop fragment + trailing slash,
/// keep the query (it carries the media id, e.g. `watch?v=…`).
pub fn canonical_url(raw: &str) -> String {
    match url::Url::parse(raw.trim()) {
        Ok(u) => {
            let host = u.host_str().unwrap_or("").to_ascii_lowercase();
            let path = u.path().trim_end_matches('/');
            match u.query() {
                Some(q) => format!("{host}{path}?{q}"),
                None => format!("{host}{path}"),
            }
        }
        Err(_) => raw.trim().to_ascii_lowercase(),
    }
}

/// A battle accepts chat submissions only while it's a true lobby — no bracket
/// generated yet — AND below the global song cap. The SAME predicate gates both the
/// synchronous admit (`evaluate`) and the async append (`add_submitted_song`), so a
/// burst can't overshoot the cap or inject orphan songs in the generate→start window.
/// (`status == Idle` is too loose: `generate_bracket` leaves the battle Idle.)
pub fn lobby_open(battle: &Battle) -> bool {
    battle.matches.is_empty() && battle.songs.len() < GLOBAL_SONG_CAP
}

/// Decide whether a raw `!submit` URL is accepted. On accept the ledger is
/// incremented (TOCTOU closed before the async fetch) and the detected source +
/// trimmed URL returned. A failed fetch later still burns the quota (no rollback).
pub fn evaluate(
    chat_submissions: bool,
    battle: Option<&Battle>,
    ledger: &mut SubmitLedger,
    user_id: &str,
    raw_url: &str,
    now_ms: u64,
) -> Option<(Source, String)> {
    if !chat_submissions {
        return None;
    }
    let battle = battle?;
    if !lobby_open(battle) {
        return None; // lobby only (no bracket yet) and under the global cap
    }
    let source = detect_source(raw_url)?; // https + known host (rejects js:/data:/…)
    let canon = canonical_url(raw_url);
    if battle
        .songs
        .iter()
        .any(|s| canonical_url(&s.source_url) == canon)
    {
        return None; // dedup — already in the lobby
    }
    if !ledger.try_admit(user_id, now_ms) {
        return None; // per-user cooldown / cap
    }
    Some((source, raw_url.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::song::Song;

    fn song(url: &str) -> Song {
        Song {
            id: "x".into(),
            title: "t".into(),
            artist: None,
            thumbnail: None,
            duration_sec: None,
            source: Source::Youtube,
            source_url: url.into(),
            submitter: None,
            metadata: None,
        }
    }

    fn idle_battle(urls: &[&str]) -> Battle {
        let mut b = Battle::new("t".into(), String::new(), String::new());
        for u in urls {
            b.add_song(song(u));
        }
        b // status defaults to Idle
    }

    #[test]
    fn ledger_cooldown_and_cap() {
        let mut l = SubmitLedger::default();
        assert!(l.try_admit("u", 0)); // first is free
        assert!(!l.try_admit("u", 1_000)); // within the 15s cooldown
        assert!(l.try_admit("u", 16_000)); // 2
        assert!(l.try_admit("u", 40_000)); // 3
        assert!(l.try_admit("u", 60_000)); // 4
        assert!(l.try_admit("u", 80_000)); // 5
        assert!(!l.try_admit("u", 200_000)); // per-user cap reached
        assert!(l.try_admit("v", 200_000)); // a different user is independent
    }

    #[test]
    fn canonical_dedup() {
        assert_eq!(
            canonical_url("https://youtu.be/x/"),
            canonical_url("https://YOUTU.BE/x")
        );
        assert_eq!(
            canonical_url("https://youtu.be/x#t=1"),
            canonical_url("https://youtu.be/x")
        );
        assert_ne!(canonical_url("https://youtu.be/x"), canonical_url("https://youtu.be/y"));
        assert_ne!(
            canonical_url("https://www.youtube.com/watch?v=a"),
            canonical_url("https://www.youtube.com/watch?v=b")
        );
    }

    #[test]
    fn evaluate_accepts_then_dedups() {
        let mut l = SubmitLedger::default();
        let b = idle_battle(&[]);
        let r = evaluate(true, Some(&b), &mut l, "u", "https://youtu.be/x", 0);
        assert!(matches!(r, Some((Source::Youtube, _))));
        // same URL (trailing-slash variant) already present → dedup, no quota burn.
        let b2 = idle_battle(&["https://youtu.be/x"]);
        assert!(evaluate(true, Some(&b2), &mut l, "w", "https://youtu.be/x/", 0).is_none());
    }

    #[test]
    fn evaluate_lobby_only() {
        use crate::domain::battle::BattleMode;
        let mut l = SubmitLedger::default();
        let mut b = idle_battle(&["https://youtu.be/a", "https://youtu.be/b"]);
        b.generate_bracket(BattleMode::Single).unwrap(); // bracket exists → not a lobby
        assert!(evaluate(true, Some(&b), &mut l, "u", "https://youtu.be/x", 0).is_none());
    }

    #[test]
    fn lobby_open_predicate() {
        use crate::domain::battle::BattleMode;
        assert!(lobby_open(&idle_battle(&[]))); // fresh lobby, under cap

        let urls: Vec<String> = (0..GLOBAL_SONG_CAP)
            .map(|i| format!("https://youtu.be/{i}"))
            .collect();
        let refs: Vec<&str> = urls.iter().map(String::as_str).collect();
        assert!(!lobby_open(&idle_battle(&refs))); // at the global cap

        // A generated bracket is no longer a lobby, even though it's still `Idle`
        // and under the cap (closes the generate→start injection window).
        let mut b = idle_battle(&["https://youtu.be/a", "https://youtu.be/b"]);
        b.generate_bracket(BattleMode::Single).unwrap();
        assert!(!lobby_open(&b));
    }

    #[test]
    fn evaluate_global_cap() {
        let mut l = SubmitLedger::default();
        let urls: Vec<String> = (0..GLOBAL_SONG_CAP)
            .map(|i| format!("https://youtu.be/{i}"))
            .collect();
        let refs: Vec<&str> = urls.iter().map(String::as_str).collect();
        let b = idle_battle(&refs);
        assert_eq!(b.songs.len(), GLOBAL_SONG_CAP);
        assert!(evaluate(true, Some(&b), &mut l, "u", "https://youtu.be/new", 0).is_none());
    }

    #[test]
    fn evaluate_setting_and_url_validation_no_quota_on_reject() {
        let mut l = SubmitLedger::default();
        let b = idle_battle(&[]);
        assert!(evaluate(false, Some(&b), &mut l, "u", "https://youtu.be/x", 0).is_none()); // disabled
        assert!(evaluate(true, None, &mut l, "u", "https://youtu.be/x", 0).is_none()); // no battle
        assert!(evaluate(true, Some(&b), &mut l, "u", "javascript:alert(1)", 0).is_none()); // unsafe
        // none of the above burned quota → the first valid one still passes at t=0.
        assert!(evaluate(true, Some(&b), &mut l, "u", "https://youtu.be/ok", 0).is_some());
    }
}
