use novex_ai_core::FoundationStatus;
use novex_rag::module;

#[test]
fn module_describes_rag_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-rag");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
