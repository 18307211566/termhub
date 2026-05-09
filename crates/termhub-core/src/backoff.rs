use std::time::Duration;

#[derive(Debug, Default, Clone)]
pub struct Backoff {
    attempts: u32,
}

impl Backoff {
    pub fn next_delay(&mut self) -> Duration {
        let secs = match self.attempts {
            0 | 1 => 1u64,
            n => {
                let exp = (n - 1).min(5);
                let v = 1u64 << exp;
                v.min(30)
            }
        };
        self.attempts = self.attempts.saturating_add(1);
        Duration::from_secs(secs)
    }

    pub fn reset(&mut self) {
        self.attempts = 0;
    }

    pub fn attempt(&self) -> u32 {
        self.attempts
    }
}
