use std::fs;
use std::path::Path;

use chrono::Timelike;
use novex_ai_core::{
    build_integration_usage_subject, can_transition_run_status, enforce_integration_usage_limits,
    foundation_modules, integration_usage_windows, normalize_task_budget, FoundationModule,
    FoundationStatus, IntegrationPrincipalType, IntegrationUsageLimitError, ResourceRef, RunStatus,
    TaskBudget, TenantContext, INTEGRATION_QPS_RESOURCE, INTEGRATION_USAGE_UNIT,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_ai_core_modules() {
    let lib = crate_file("src/lib.rs");

    for module in [
        "budget",
        "context",
        "integration_usage",
        "module",
        "run_graph",
    ] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct FoundationModule",
        "pub struct TenantContext",
        "pub struct IntegrationUsageSubject",
        "pub enum RunStatus",
        "pub struct TaskBudget",
        "pub fn normalize_task_budget",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn ai_core_domain_modules_exist() {
    for module in [
        "src/budget.rs",
        "src/context.rs",
        "src/integration_usage.rs",
        "src/module.rs",
        "src/run_graph.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_core_contracts() {
    let module = FoundationModule::skeleton("run-graph", "Run Graph", "core", "runs");
    assert_eq!(module.status, FoundationStatus::Skeleton);
    assert!(foundation_modules()
        .iter()
        .any(|module| module.id == "run-graph"));

    let tenant = TenantContext {
        tenant_id: "tenant-1".to_owned(),
        user_id: Some("user-1".to_owned()),
        role_ids: vec!["admin".to_owned()],
    };
    let resource = ResourceRef {
        resource_type: "dataset".to_owned(),
        resource_id: "42".to_owned(),
        tenant_id: tenant.tenant_id.clone(),
    };
    assert_eq!(resource.tenant_id, "tenant-1");

    let subject =
        build_integration_usage_subject(IntegrationPrincipalType::ApiKey, 11, "42", 2, 5).unwrap();
    assert_eq!(subject.scope_type, "api_key");
    assert_eq!(
        enforce_integration_usage_limits(&subject, 3, 5).unwrap_err(),
        IntegrationUsageLimitError::QpsExceeded
    );

    let now = chrono::DateTime::parse_from_rfc3339("2026-06-06T08:09:10Z")
        .unwrap()
        .naive_utc();
    let windows = integration_usage_windows(now);
    assert_eq!(windows[0].resource_type, INTEGRATION_QPS_RESOURCE);
    assert_eq!(windows[0].usage_unit, INTEGRATION_USAGE_UNIT);
    assert_eq!(windows[0].window_start.second(), 10);

    assert!(can_transition_run_status(
        RunStatus::Queued,
        RunStatus::Running
    ));
    assert!(RunStatus::Succeeded.is_terminal());

    let budget = normalize_task_budget(TaskBudget {
        max_steps: Some(3),
        max_tool_calls: Some(1),
        max_seconds: None,
        max_cost_cents: None,
    })
    .unwrap();
    assert_eq!(budget.max_seconds, Some(120));
}
