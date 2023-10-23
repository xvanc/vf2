use core::time::Duration;

const MICROS_PER_SECOND: u64 = 1000000;
const TIME_BASE_FREQUENCY: u64 = 4000000;

#[derive(Clone, Copy)]
pub struct Instant(u64);

impl Instant {
    pub fn now() -> Instant {
        let mtime = 0x200bff8 as *mut u64;
        Instant(unsafe { mtime.read_volatile() })
    }

    pub fn duration_since(self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    pub fn checked_duration_since(self, earlier: Instant) -> Option<Duration> {
        let ticks_per_micro = (TIME_BASE_FREQUENCY + MICROS_PER_SECOND - 1) / MICROS_PER_SECOND;

        let diff = self.0.checked_sub(earlier.0)?;

        let secs = diff / TIME_BASE_FREQUENCY;
        let rems = diff % TIME_BASE_FREQUENCY;
        let nanos = (rems / ticks_per_micro) * 1000;

        Some(Duration::new(secs, nanos as u32))
    }
}

impl core::ops::Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        self.duration_since(rhs)
    }
}

#[derive(Clone, Copy)]
pub struct Timeout {
    start: Instant,
    duration: Duration,
}

impl Timeout {
    pub fn start(duration: Duration) -> Timeout {
        Timeout {
            start: Instant::now(),
            duration,
        }
    }

    pub fn expired(&self) -> bool {
        Instant::now() - self.start >= self.duration
    }
}
