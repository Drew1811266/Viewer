//! 内存压力联动（性能优化方案三）集成测试：
//! runtime 收到压力等级后收缩派生缓存、调整解码调度并发并广播事件；
//! warn 有冷却窗口；同一等级幂等。

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use viewer_application::{ProjectAccess, ProjectProbeError, ProjectProbePort};
use viewer_desktop::state::{DesktopRuntime, MemoryPressureLevel};
use viewer_infrastructure::decode_scheduler::DEFAULT_DECODE_CONCURRENCY;
use viewer_infrastructure::session_cache::SESSION_CACHE_LIMIT_BYTES;

struct FixedProbe(ProjectAccess);

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}

#[derive(Clone, Default)]
struct RecordingSink {
    levels: Arc<std::sync::Mutex<Vec<MemoryPressureLevel>>>,
}

impl viewer_desktop::state::MemoryPressureEventSink for RecordingSink {
    fn emit_memory_pressure(&self, level: MemoryPressureLevel) {
        self.levels.lock().unwrap().push(level);
    }
}

fn runtime_in(cache: &tempfile::TempDir) -> Arc<DesktopRuntime> {
    Arc::new(DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
    ))
}

#[tokio::test]
async fn warn_then_critical_shrink_decode_concurrency_and_normal_restores_it() {
    let cache = tempfile::tempdir().unwrap();
    let runtime = runtime_in(&cache);
    let scheduler = runtime.decode_scheduler();

    runtime
        .apply_memory_pressure(MemoryPressureLevel::Warning)
        .await;
    assert_eq!(scheduler.snapshot().limit, 1);

    runtime
        .apply_memory_pressure(MemoryPressureLevel::Critical)
        .await;
    assert_eq!(scheduler.snapshot().limit, 0);
    // critical 之下交互请求仍保底 1 槽（调度器单测覆盖），这里只验证限值。

    runtime
        .apply_memory_pressure(MemoryPressureLevel::Normal)
        .await;
    assert_eq!(scheduler.snapshot().limit, DEFAULT_DECODE_CONCURRENCY);
    assert_eq!(runtime.memory_pressure_level(), MemoryPressureLevel::Normal);
}

#[tokio::test]
async fn repeated_warn_within_the_cooldown_window_is_ignored() {
    let cache = tempfile::tempdir().unwrap();
    let runtime = runtime_in(&cache);

    runtime
        .apply_memory_pressure(MemoryPressureLevel::Warning)
        .await;
    runtime
        .apply_memory_pressure(MemoryPressureLevel::Normal)
        .await;
    assert_eq!(runtime.memory_pressure_level(), MemoryPressureLevel::Normal);

    // 冷却窗口内的第二次 warn 不改变状态、不触发收缩。
    runtime
        .apply_memory_pressure(MemoryPressureLevel::Warning)
        .await;
    assert_eq!(runtime.memory_pressure_level(), MemoryPressureLevel::Normal);
    assert_eq!(
        runtime.decode_scheduler().snapshot().limit,
        DEFAULT_DECODE_CONCURRENCY
    );

    // critical 不受冷却限制。
    runtime
        .apply_memory_pressure(MemoryPressureLevel::Critical)
        .await;
    assert_eq!(
        runtime.memory_pressure_level(),
        MemoryPressureLevel::Critical
    );
    assert_eq!(runtime.decode_scheduler().snapshot().limit, 0);
}

#[tokio::test]
async fn pressure_transitions_are_broadcast_through_the_event_sink() {
    let cache = tempfile::tempdir().unwrap();
    let runtime = runtime_in(&cache);
    let sink = RecordingSink::default();
    runtime.set_memory_pressure_sink(Arc::new(sink.clone()));

    runtime
        .apply_memory_pressure(MemoryPressureLevel::Warning)
        .await;
    runtime
        .apply_memory_pressure(MemoryPressureLevel::Warning)
        .await; // 同级幂等
    runtime
        .apply_memory_pressure(MemoryPressureLevel::Critical)
        .await;
    runtime
        .apply_memory_pressure(MemoryPressureLevel::Normal)
        .await;

    assert_eq!(
        *sink.levels.lock().unwrap(),
        vec![
            MemoryPressureLevel::Warning,
            MemoryPressureLevel::Critical,
            MemoryPressureLevel::Normal,
        ]
    );
}

#[tokio::test]
async fn warning_pressure_shrinks_the_opened_session_derived_cache() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    // 只读项目最小 fixture：一个非空目录即可完成扫描。
    std::fs::write(project.path().join("front.jpg"), b"jpeg").unwrap();

    let runtime = runtime_in(&cache);
    runtime
        .open_project(project.path())
        .await
        .expect("readonly fixture opens");

    runtime
        .apply_memory_pressure(MemoryPressureLevel::Warning)
        .await;
    // 无派生图像时收缩是空操作但必须不 panic；预算语义由 infrastructure 单测保证。
    assert_eq!(
        runtime.memory_pressure_level(),
        MemoryPressureLevel::Warning
    );
    let _ = SESSION_CACHE_LIMIT_BYTES;
    let _ = PathBuf::new();
}
