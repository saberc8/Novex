use novex_ai_core::{validate_run_transition, RunStatus};

#[test]
fn run_graph_status_transition_allows_approval_resume_success_path() {
    let path = [
        RunStatus::Queued,
        RunStatus::Running,
        RunStatus::WaitingApproval,
        RunStatus::Resuming,
        RunStatus::Running,
        RunStatus::Succeeded,
    ];

    for window in path.windows(2) {
        validate_run_transition(window[0], window[1]).unwrap();
    }
}

#[test]
fn run_graph_status_transition_allows_cancel_from_active_states() {
    for status in [
        RunStatus::Queued,
        RunStatus::Running,
        RunStatus::WaitingApproval,
        RunStatus::Paused,
        RunStatus::Resuming,
    ] {
        validate_run_transition(status, RunStatus::Cancelling).unwrap();
        validate_run_transition(RunStatus::Cancelling, RunStatus::Cancelled).unwrap();
    }
}

#[test]
fn run_graph_status_transition_rejects_terminal_restart() {
    let err = validate_run_transition(RunStatus::Succeeded, RunStatus::Running).unwrap_err();

    assert_eq!(err.from, RunStatus::Succeeded);
    assert_eq!(err.to, RunStatus::Running);
    assert!(RunStatus::Succeeded.is_terminal());
}
