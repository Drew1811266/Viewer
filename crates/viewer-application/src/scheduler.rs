use std::sync::atomic::{AtomicU64, Ordering};
use viewer_domain::search::Generation;

#[derive(Debug, Default)]
pub struct GenerationGuard {
    current: AtomicU64,
}

impl GenerationGuard {
    pub fn current(&self) -> Generation {
        Generation::new(self.current.load(Ordering::SeqCst))
    }

    pub fn bump(&self) -> Generation {
        Generation::new(self.current.fetch_add(1, Ordering::SeqCst) + 1)
    }

    pub fn is_current(&self, generation: Generation) -> bool {
        self.current.load(Ordering::SeqCst) == generation.get()
    }
}

#[cfg(test)]
mod tests {
    use super::GenerationGuard;

    #[test]
    fn old_generation_cannot_publish_after_bump() {
        let guard = GenerationGuard::default();
        let old = guard.current();
        let current = guard.bump();
        assert!(!guard.is_current(old));
        assert!(guard.is_current(current));
    }
}
