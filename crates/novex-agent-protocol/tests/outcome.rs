use novex_agent_protocol::TurnOutcome;

#[test]
fn turn_outcome_identifies_terminal_states() {
    assert!(TurnOutcome::Final.is_terminal());
    assert!(TurnOutcome::Paused.is_terminal());
    assert!(!TurnOutcome::NeedsFollowUp.is_terminal());
}
