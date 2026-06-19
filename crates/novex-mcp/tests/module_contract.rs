use novex_ai_core::FoundationStatus;
use novex_mcp::module;

#[test]
fn module_describes_mcp_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-mcp");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
