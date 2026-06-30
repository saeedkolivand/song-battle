use serde::{Deserialize, Serialize};

/// Server-authoritative countdown for the active match. The broadcaster calls
/// `tick()` once a second; the frontend just renders `remaining_sec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timer {
    pub duration_sec: u32,
    pub remaining_sec: u32,
    pub running: bool,
}

impl Timer {
    pub fn new(duration_sec: u32) -> Self {
        Self {
            duration_sec,
            remaining_sec: duration_sec,
            running: false,
        }
    }

    /// (Re)start at full duration.
    pub fn start(&mut self) {
        self.remaining_sec = self.duration_sec;
        self.running = true;
    }

    /// Advance one second. Returns true on the tick that reaches zero.
    pub fn tick(&mut self) -> bool {
        if !self.running {
            return false;
        }
        self.remaining_sec = self.remaining_sec.saturating_sub(1);
        if self.remaining_sec == 0 {
            self.running = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_then_expires() {
        let mut t = Timer::new(2);
        assert!(!t.running);
        t.start();
        assert!(t.running && t.remaining_sec == 2);
        assert!(!t.tick()); // 2 -> 1
        assert_eq!(t.remaining_sec, 1);
        assert!(t.tick()); // 1 -> 0, expired
        assert!(!t.running);
        assert!(!t.tick()); // stays expired, no double-fire
    }
}
