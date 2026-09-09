use viewer_desktop::image_render_runtime::{NativeRecoveryDecision, NativeRecoveryTracker};

#[test]
fn one_retryable_failure_recovers_natively_and_two_consecutive_failures_escalate() {
    let mut recovery = NativeRecoveryTracker::default();

    assert_eq!(
        recovery.record_retryable_failure(),
        NativeRecoveryDecision::RecoverNative
    );
    assert_eq!(recovery.recovery_count(), 1);
    assert_eq!(
        recovery.record_retryable_failure(),
        NativeRecoveryDecision::Escalate
    );
    assert_eq!(recovery.recovery_count(), 2);

    recovery.record_presented_frame();
    assert_eq!(
        recovery.record_retryable_failure(),
        NativeRecoveryDecision::RecoverNative
    );
    assert_eq!(recovery.recovery_count(), 3);
}

#[test]
fn terminal_failure_never_enters_the_retry_loop() {
    let mut recovery = NativeRecoveryTracker::default();
    recovery.record_retryable_failure();
    recovery.record_terminal_failure();

    assert_eq!(recovery.consecutive_failures(), 0);
    assert_eq!(recovery.recovery_count(), 1);
}
