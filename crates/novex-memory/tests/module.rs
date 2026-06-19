use novex_ai_core::FoundationStatus;
use novex_memory::module;

#[test]
fn module_describes_memory_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-memory");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
