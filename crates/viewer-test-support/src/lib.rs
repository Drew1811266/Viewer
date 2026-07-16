use viewer_application::ClockPort;

pub struct FixedClock(i64);

impl FixedClock {
    pub fn new(value: i64) -> Self {
        Self(value)
    }
}

impl ClockPort for FixedClock {
    fn unix_millis(&self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::FixedClock;
    use viewer_application::ClockPort;

    #[test]
    fn fixed_clock_returns_its_configured_value() {
        assert_eq!(FixedClock::new(42).unix_millis(), 42);
    }
}
