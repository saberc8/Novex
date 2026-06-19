use novex_ai_core::FoundationStatus;
use novex_connectors::module;

#[test]
fn module_describes_connector_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-connectors");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
