use novex_ai_core::{foundation_modules, FoundationStatus};

#[test]
fn foundation_modules_describe_m0_skeleton_boundaries() {
    let modules = foundation_modules();

    assert!(modules.iter().any(|module| module.id == "run-graph"));
    assert!(modules.iter().any(|module| module.id == "policy"));
    assert!(modules
        .iter()
        .all(|module| module.status == FoundationStatus::Skeleton));
}
