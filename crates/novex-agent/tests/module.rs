use novex_agent::module;
use novex_ai_core::FoundationStatus;

#[test]
fn module_describes_agent_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-agent");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
