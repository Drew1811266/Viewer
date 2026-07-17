pub use viewer_application::{FaultInjector, InjectedCrash, NoFaults};
use viewer_domain::{OperationId, operation::OperationState};

#[derive(Clone, Copy, Debug)]
pub struct FailAfterState(pub OperationState);

impl FaultInjector for FailAfterState {
    fn after_persist(
        &self,
        operation_id: OperationId,
        state: OperationState,
    ) -> Result<(), InjectedCrash> {
        if state == self.0 {
            Err(InjectedCrash {
                operation_id,
                state,
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FailAfterState, FaultInjector};
    use viewer_domain::{OperationId, operation::OperationState};

    #[test]
    fn fail_after_state_matches_only_configured_commit() {
        let injector = FailAfterState(OperationState::Verified);
        let operation_id = OperationId::new();
        assert!(
            injector
                .after_persist(operation_id, OperationState::FsApplied)
                .is_ok()
        );
        assert_eq!(
            injector
                .after_persist(operation_id, OperationState::Verified)
                .unwrap_err()
                .state,
            OperationState::Verified
        );
    }
}
