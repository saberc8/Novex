use novex_ai_core::FoundationStatus;
use novex_tools::module;

#[test]
fn module_describes_tool_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-tools");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
