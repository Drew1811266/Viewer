pub mod image_cache;
pub mod operation;
pub mod portable;
pub mod scan;
pub mod search;
pub mod session_cache;
pub mod settings;
pub mod text;
pub mod video_probe;

use viewer_application::ClockPort;

pub struct SystemClock;

impl ClockPort for SystemClock {
    fn unix_millis(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_millis() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::SystemClock;
    use std::time::{SystemTime, UNIX_EPOCH};
    use viewer_application::ClockPort;

    #[test]
    fn system_clock_returns_the_current_unix_time_in_milliseconds() {
        let before = unix_millis();
        let actual = SystemClock.unix_millis();
        let after = unix_millis();

        assert!(
            (before..=after).contains(&actual),
            "clock value {actual} was not between {before} and {after}"
        );
    }

    fn unix_millis() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_millis() as i64
    }
}
