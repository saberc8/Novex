use novex_ai_core::FoundationStatus;
use novex_plugin::module;

#[test]
fn module_describes_plugin_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-plugin");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
