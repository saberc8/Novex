use novex_ai_core::FoundationStatus;
use novex_model::module;

#[test]
fn module_describes_model_boundary() {
    let module = module();

    assert_eq!(module.id, "novex-model");
    assert_eq!(module.status, FoundationStatus::Skeleton);
}
