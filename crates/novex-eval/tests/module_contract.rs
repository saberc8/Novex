use novex_ai_core::FoundationStatus;
use novex_eval::module;

#[test]
fn module_describes_eval_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-eval");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
