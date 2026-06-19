use novex_ai_core::FoundationStatus;
use novex_trace::module;

#[test]
fn module_describes_trace_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-trace");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
