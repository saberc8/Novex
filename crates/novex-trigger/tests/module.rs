use novex_ai_core::FoundationStatus;
use novex_trigger::module;

#[test]
fn module_describes_trigger_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-trigger");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
