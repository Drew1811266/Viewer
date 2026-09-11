use serde::Serialize;

/// `viewer://memory-pressure` 事件载荷。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPressureLevelDto {
    Normal,
    Warn,
    Critical,
}

impl From<viewer_infrastructure::session_cache::MemoryPressureLevel> for MemoryPressureLevelDto {
    fn from(level: viewer_infrastructure::session_cache::MemoryPressureLevel) -> Self {
        match level {
            viewer_infrastructure::session_cache::MemoryPressureLevel::Normal => Self::Normal,
            viewer_infrastructure::session_cache::MemoryPressureLevel::Warning => Self::Warn,
            viewer_infrastructure::session_cache::MemoryPressureLevel::Critical => Self::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryPressureLevelDto;
    use viewer_infrastructure::session_cache::MemoryPressureLevel;

    #[test]
    fn memory_pressure_level_serializes_the_exact_ipc_shape() {
        assert_eq!(
            serde_json::to_value(MemoryPressureLevelDto::from(MemoryPressureLevel::Normal))
                .unwrap(),
            serde_json::json!("normal")
        );
        assert_eq!(
            serde_json::to_value(MemoryPressureLevelDto::from(MemoryPressureLevel::Warning))
                .unwrap(),
            serde_json::json!("warn")
        );
        assert_eq!(
            serde_json::to_value(MemoryPressureLevelDto::from(MemoryPressureLevel::Critical))
                .unwrap(),
            serde_json::json!("critical")
        );
    }
}
