use crate::error::{CommandError, ErrorCategory};
use crate::state::{DesktopRuntime, MemoryPressureLevel};
use std::sync::Arc;
use tauri::State;

/// 仅供调试与压力联动手动验证（仅 debug 构建注册）：
/// 注入一次内存压力等级，走与真实 dispatch 源完全相同的联动链路。
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_simulate_memory_pressure(
    runtime: State<'_, Arc<DesktopRuntime>>,
    level: String,
) -> Result<(), CommandError> {
    let level = parse_memory_pressure_level(&level)?;
    runtime.apply_memory_pressure(level).await;
    Ok(())
}

#[cfg(debug_assertions)]
fn parse_memory_pressure_level(raw: &str) -> Result<MemoryPressureLevel, CommandError> {
    match raw {
        "normal" => Ok(MemoryPressureLevel::Normal),
        "warn" => Ok(MemoryPressureLevel::Warning),
        "critical" => Ok(MemoryPressureLevel::Critical),
        other => Err(CommandError::new(
            "invalid_memory_pressure_level",
            ErrorCategory::Internal,
            &format!("unknown memory pressure level: {other}"),
            false,
        )),
    }
}
