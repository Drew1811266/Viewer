use std::time::Instant;

use serde::Serialize;

pub(super) struct CpuSample {
    cpu_ns: u64,
    at: Instant,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum CpuObservation {
    Available {
        method: &'static str,
        scope: &'static str,
        cpu_time_ms: f64,
        observation_ms: f64,
        one_core_percent: f64,
    },
    Unavailable {
        reason: String,
    },
}

fn clock_nanoseconds(seconds: i64, nanoseconds: i64) -> Result<u64, &'static str> {
    let seconds = u64::try_from(seconds).map_err(|_| "negative CPU clock seconds")?;
    let nanoseconds = u64::try_from(nanoseconds).map_err(|_| "negative CPU clock nanoseconds")?;
    if nanoseconds >= 1_000_000_000 {
        return Err("CPU clock nanoseconds out of range");
    }
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(nanoseconds))
        .ok_or("CPU clock nanoseconds overflow")
}

#[cfg(target_os = "macos")]
fn read_thread_cpu_ns() -> Result<u64, String> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: value is valid writable storage. This clock measures user and
    // kernel CPU of the calling thread, not process CPU or elapsed wall time.
    if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut value) } != 0 {
        return Err(format!(
            "thread CPU clock unavailable: {}",
            std::io::Error::last_os_error()
        ));
    }
    clock_nanoseconds(value.tv_sec, value.tv_nsec).map_err(str::to_owned)
}

#[cfg(target_os = "macos")]
fn sample_on_main(_main_thread: objc2::MainThreadMarker) -> Result<CpuSample, String> {
    Ok(CpuSample {
        cpu_ns: read_thread_cpu_ns()?,
        at: Instant::now(),
    })
}

/// Only the measurement worker calls this, twice around its warm workload.
/// AppKit must be running its event loop; no render/input hot path waits here.
#[cfg(target_os = "macos")]
pub(super) fn capture() -> Result<CpuSample, String> {
    dispatch2::run_on_main(sample_on_main)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn capture() -> Result<CpuSample, String> {
    Err("AppKit main-thread CPU observation requires macOS".into())
}

pub(super) fn observe(
    start: Result<CpuSample, String>,
    end: Result<CpuSample, String>,
) -> CpuObservation {
    let (start, end) = match (start, end) {
        (Ok(start), Ok(end)) => (start, end),
        (Err(reason), _) | (_, Err(reason)) => return CpuObservation::Unavailable { reason },
    };
    let Some(wall) = end
        .at
        .checked_duration_since(start.at)
        .filter(|wall| !wall.is_zero())
    else {
        return CpuObservation::Unavailable {
            reason: "CPU observation wall interval is not positive".into(),
        };
    };
    let Some(cpu_ns) = end.cpu_ns.checked_sub(start.cpu_ns) else {
        return CpuObservation::Unavailable {
            reason: "thread CPU clock moved backwards".into(),
        };
    };
    CpuObservation::Available {
        method: "clock_gettime_thread_cpu_on_appkit_main",
        scope: "warm_240_frame_workload_bracket",
        cpu_time_ms: cpu_ns as f64 / 1_000_000.0,
        observation_ms: wall.as_secs_f64() * 1000.0,
        one_core_percent: (cpu_ns as f64 / 1_000_000_000.0) / wall.as_secs_f64() * 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn observation(cpu_start: u64, cpu_end: u64, wall: Duration) -> serde_json::Value {
        let start = Instant::now();
        serde_json::to_value(observe(
            Ok(CpuSample {
                cpu_ns: cpu_start,
                at: start,
            }),
            Ok(CpuSample {
                cpu_ns: cpu_end,
                at: start + wall,
            }),
        ))
        .unwrap()
    }

    #[test]
    fn converts_cpu_clock_without_truncation_or_overflow() {
        assert_eq!(clock_nanoseconds(3, 42), Ok(3_000_000_042));
        assert_eq!(clock_nanoseconds(0, 0), Ok(0));
        assert_eq!(clock_nanoseconds(18_446_744_073, 709_551_615), Ok(u64::MAX));
        for (seconds, nanos) in [
            (-1, 0),
            (0, -1),
            (0, 1_000_000_000),
            (i64::MAX, 0),
            (18_446_744_073, 709_551_616),
        ] {
            assert!(clock_nanoseconds(seconds, nanos).is_err());
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn supported_clock_is_readable_and_monotonic_on_this_test_thread() {
        // Deliberately no capture(): libtest has no AppKit main event loop.
        // This checks the OS clock, not a real-window/main-thread measurement.
        let before = read_thread_cpu_ns().unwrap();
        let after = read_thread_cpu_ns().unwrap();
        assert!(after >= before);
    }

    #[test]
    fn reports_thread_cpu_separately_from_bracket_wall_time() {
        let value = observation(100_000_000, 104_000_000, Duration::from_millis(20));
        assert_eq!(value["status"], "available");
        assert_eq!(value["method"], "clock_gettime_thread_cpu_on_appkit_main");
        assert_eq!(value["scope"], "warm_240_frame_workload_bracket");
        assert_eq!(value["cpu_time_ms"], 4.0);
        assert_eq!(value["observation_ms"], 20.0);
        assert_eq!(value["one_core_percent"], 20.0);
    }

    #[test]
    fn genuine_zero_cpu_usage_is_not_a_missing_measurement() {
        let value = observation(42, 42, Duration::from_millis(10));
        assert_eq!(value["status"], "available");
        assert_eq!(value["observation_ms"], 10.0);
        assert_eq!(value["cpu_time_ms"], 0.0);
    }

    #[test]
    fn invalid_or_missing_samples_are_unavailable_without_numeric_zeros() {
        let now = Instant::now();
        let values = [
            observation(2, 1, Duration::from_millis(10)),
            observation(1, 2, Duration::ZERO),
            serde_json::to_value(observe(
                Ok(CpuSample {
                    cpu_ns: 1,
                    at: now + Duration::from_millis(1),
                }),
                Ok(CpuSample { cpu_ns: 2, at: now }),
            ))
            .unwrap(),
            serde_json::to_value(observe(
                Err("start clock unavailable".into()),
                Ok(CpuSample { cpu_ns: 2, at: now }),
            ))
            .unwrap(),
            serde_json::to_value(observe(
                Ok(CpuSample { cpu_ns: 1, at: now }),
                Err("end clock unavailable".into()),
            ))
            .unwrap(),
        ];
        for value in values {
            assert_eq!(value["status"], "unavailable");
            assert!(
                value["reason"]
                    .as_str()
                    .is_some_and(|reason| !reason.is_empty())
            );
            assert!(value.get("cpu_time_ms").is_none());
            assert!(value.get("observation_ms").is_none());
            assert!(value.get("one_core_percent").is_none());
        }
    }
}
