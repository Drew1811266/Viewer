//! 解码调度器性能门禁（性能优化方案四，见 docs/architecture/performance-optimization-plan.md §5）。
//!
//! 门控 `VIEWER_RUN_PERF_TESTS=1`，不在默认 CI 跑；由 `pnpm verify:perf` 显式执行，
//! 结果记入 `docs/quality/perf-baseline.md`。
//!
//! 场景与预算（M 系列基准机）：
//! - 缩略图风暴：200 个缩略图请求经 limit=3 的调度器，并发峰值必须等于 limit，
//!   总耗时 ≤ 串行（limit=1 同负载）的 1.6×；
//! - 取消代价：风暴进行到一半时取消全部未完成任务（模拟反向滚动 abort），
//!   之后 100ms 内必须有槽位可被新请求获取，且不遗留任何许可（ticket 泄漏检测）。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use viewer_infrastructure::decode_scheduler::{DecodePriority, DecodeScheduler};

const STORM_TASKS: usize = 200;
const STORM_DECODE_MS: u64 = 2;
const CANCEL_STORM_DECODE_MS: u64 = 10;
const SLOT_RECLAIM_BUDGET: Duration = Duration::from_millis(100);

async fn run_storm(limit: usize) -> (Duration, usize) {
    let scheduler = DecodeScheduler::new(limit);
    let in_flight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();
    let mut handles = Vec::with_capacity(STORM_TASKS);
    for _ in 0..STORM_TASKS {
        let scheduler = Arc::clone(&scheduler);
        let in_flight = Arc::clone(&in_flight);
        let peak = Arc::clone(&peak);
        handles.push(tokio::spawn(async move {
            let _ticket = scheduler.acquire(DecodePriority::Interactive).await;
            let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(STORM_DECODE_MS)).await;
            in_flight.fetch_sub(1, Ordering::SeqCst);
        }));
    }
    for handle in handles {
        handle.await.unwrap();
    }
    let stats = scheduler.snapshot();
    assert_eq!(
        stats.in_flight + stats.waiting_interactive + stats.waiting_prefetch,
        0,
        "storm must drain every queued or in-flight ticket"
    );
    (started.elapsed(), peak.load(Ordering::SeqCst))
}

// Break caught: a priority queue that silently serializes (peak < limit) or a
// broken admission path that lets concurrency exceed the configured limit.
#[tokio::test]
async fn thumbnail_storm_peak_matches_limit_and_beats_serial_within_budget() {
    if std::env::var_os("VIEWER_RUN_PERF_TESTS").is_none() {
        return;
    }
    let (serial_elapsed, serial_peak) = run_storm(1).await;
    let (storm_elapsed, storm_peak) = run_storm(3).await;

    assert_eq!(
        serial_peak, 1,
        "serial reference must run one decode at a time"
    );
    assert_eq!(
        storm_peak, 3,
        "storm concurrency peak must equal the scheduler limit"
    );
    assert!(
        storm_elapsed.as_secs_f64() <= serial_elapsed.as_secs_f64() * 1.6,
        "storm elapsed {:?} must stay within 1.6x of the serial reference {:?}",
        storm_elapsed,
        serial_elapsed
    );
    eprintln!(
        "缩略图风暴: 串行(limit=1)={serial_elapsed:?} 风暴(limit=3)={storm_elapsed:?} 峰值={storm_peak}"
    );
}

// Break caught: cancelled waiters keeping queue slots reserved (stalling later
// work) or a dropped ticket leaking an in-flight slot forever.
#[tokio::test]
async fn mid_storm_cancellation_reclaims_a_slot_within_100ms() {
    if std::env::var_os("VIEWER_RUN_PERF_TESTS").is_none() {
        return;
    }
    let scheduler = DecodeScheduler::new(3);
    let mut handles = Vec::with_capacity(STORM_TASKS);
    for _ in 0..STORM_TASKS {
        let scheduler = Arc::clone(&scheduler);
        handles.push(tokio::spawn(async move {
            let _ticket = scheduler.acquire(DecodePriority::Interactive).await;
            tokio::time::sleep(Duration::from_millis(CANCEL_STORM_DECODE_MS)).await;
        }));
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut cancelled = 0usize;
    for handle in &handles {
        if !handle.is_finished() {
            handle.abort();
            cancelled += 1;
        }
    }
    assert!(
        cancelled > 0,
        "cancellation must hit at least one pending task"
    );

    let started = Instant::now();
    let admitted = tokio::time::timeout(SLOT_RECLAIM_BUDGET, async {
        scheduler.acquire(DecodePriority::Interactive).await
    })
    .await;
    let reclaim = started.elapsed();
    let ticket = admitted.unwrap_or_else(|_| {
        panic!(
            "a slot must be reclaimable within {SLOT_RECLAIM_BUDGET:?} after cancellation; took {reclaim:?}"
        )
    });
    drop(ticket);

    let stats = scheduler.snapshot();
    assert!(
        stats.cancelled_while_waiting > 0,
        "cancelled waiters must be counted in telemetry"
    );
    for handle in handles {
        let _ = handle.await;
    }
    let drained = scheduler.snapshot();
    assert_eq!(
        drained.in_flight + drained.waiting_interactive + drained.waiting_prefetch,
        0,
        "no ticket may leak after the cancelled storm fully unwinds"
    );
    eprintln!(
        "取消代价: 取消 {cancelled} 个未完成任务，槽位回收耗时 {reclaim:?}（预算 {SLOT_RECLAIM_BUDGET:?}），排队中取消计数 {}",
        stats.cancelled_while_waiting
    );
}
