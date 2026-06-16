use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::{process::Command, time::timeout};

use crate::{
    application::system::ensure_max_chars,
    infrastructure::persistence::ai_template_repository::{
        AiTemplateRepository, CustomerPackageApplyRecord, CustomerPackageConnectorApplyRecord,
        CustomerPackageEvalSetApplyRecord, CustomerPackageFrontendConfigApplyRecord,
        CustomerPackageMenuApplyRecord, CustomerPackagePluginApplyRecord,
        CustomerPackageRoleApplyRecord, CustomerPackageSkillApplyRecord,
        CustomerPackageTriggerApplyRecord, TemplateSmokeResultSaveRecord,
        TemplateSmokeRunSaveRecord,
    },
    shared::{
        error::AppError,
        id::next_id,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_TEMPLATE_PAGE_SIZE: u64 = 20;
const ENABLED_STATUS: i16 = 1;
const DEFAULT_SMOKE_TIMEOUT_SECS: u64 = 300;
const TEMPLATE_MANIFESTS: [&str; 5] = [
    include_str!("../../../../templates/llm-chat/template.json"),
    include_str!("../../../../templates/knowledge-base-chat/template.json"),
    include_str!("../../../../templates/agent-workspace/template.json"),
    include_str!("../../../../templates/customer-service-agent/template.json"),
    include_str!("../../../../templates/training-app/template.json"),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryTemplateQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_template_size")]
    pub size: u64,
    #[serde(default = "default_enabled_status")]
    pub status: Option<i16>,
    #[serde(default)]
    pub category: Option<String>,
}

impl Default for DeliveryTemplateQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            size: DEFAULT_TEMPLATE_PAGE_SIZE,
            status: Some(ENABLED_STATUS),
            category: None,
        }
    }
}

impl DeliveryTemplateQuery {
    fn page_query(&self) -> PageQuery {
        PageQuery {
            page: self.page,
            size: self.size,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryTemplate {
    pub code: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub frontend_entry: String,
    #[serde(default)]
    pub frontend_app: String,
    #[serde(default)]
    pub frontend_pages: Vec<TemplateFrontendPage>,
    #[serde(default)]
    pub smoke_checks: Vec<TemplateSmokeCheck>,
    pub smoke_script: String,
    pub sort: i32,
    pub status: i16,
    pub branding: TemplateBranding,
    #[serde(default)]
    pub roles: Vec<TemplateRole>,
    #[serde(default)]
    pub menus: Vec<TemplateMenu>,
    #[serde(default)]
    pub prompts: Vec<TemplatePrompt>,
    #[serde(default)]
    pub skills: Vec<TemplateSkill>,
    #[serde(default)]
    pub connectors: Vec<TemplateConnector>,
    #[serde(default)]
    pub plugins: Vec<TemplatePlugin>,
    #[serde(default)]
    pub triggers: Vec<TemplateTrigger>,
    #[serde(default)]
    pub eval_sets: Vec<TemplateEvalSet>,
    #[serde(default)]
    pub deployment_checklist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateBranding {
    pub brand_name: String,
    pub logo_text: String,
    pub primary_color: String,
    pub public_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFrontendPage {
    pub code: String,
    pub title: String,
    pub path: String,
    pub nav_label: String,
    pub permission: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSmokeCheck {
    pub code: String,
    pub name: String,
    pub workdir: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRole {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateMenu {
    pub code: String,
    pub title: String,
    pub path: String,
    pub permission: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePrompt {
    pub code: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSkill {
    pub code: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateConnector {
    pub code: String,
    pub name: String,
    pub kind: String,
    pub auth_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePlugin {
    pub code: String,
    pub name: String,
    pub runtime: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateTrigger {
    pub code: String,
    pub name: String,
    pub source_type: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateEvalSet {
    pub code: String,
    pub name: String,
    pub case_count: i32,
    #[serde(default)]
    pub metrics: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerPackageCommand {
    pub template_code: String,
    pub customer_name: String,
    pub app_name: String,
    #[serde(default)]
    pub industry: Option<String>,
    #[serde(default)]
    pub brand_name: Option<String>,
    #[serde(default)]
    pub primary_color: Option<String>,
    #[serde(default)]
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerTenantConfig {
    pub customer_name: String,
    pub app_name: String,
    pub industry: String,
    pub template_code: String,
    pub frontend_entry: String,
    pub frontend_app: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerFrontendConfig {
    pub app: String,
    pub entry: String,
    pub entry_url: String,
    pub branding: TemplateBranding,
    pub default_page: TemplateFrontendPage,
    pub navigation: Vec<TemplateFrontendPage>,
    pub allowed_roles: Vec<TemplateRole>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerProvisioningPlan {
    pub plan_id: String,
    pub mode: String,
    pub tenant_code: String,
    pub idempotency_key: String,
    pub steps: Vec<CustomerProvisioningStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerProvisioningStep {
    pub code: String,
    pub title: String,
    pub target: String,
    pub operation: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerPackageResp {
    pub package_id: String,
    pub template: DeliveryTemplate,
    pub tenant_config: CustomerTenantConfig,
    pub branding: TemplateBranding,
    pub frontend_config: CustomerFrontendConfig,
    pub provisioning_plan: CustomerProvisioningPlan,
    pub roles: Vec<TemplateRole>,
    pub menus: Vec<TemplateMenu>,
    pub frontend_pages: Vec<TemplateFrontendPage>,
    pub prompts: Vec<TemplatePrompt>,
    pub skills: Vec<TemplateSkill>,
    pub connectors: Vec<TemplateConnector>,
    pub plugins: Vec<TemplatePlugin>,
    pub triggers: Vec<TemplateTrigger>,
    pub eval_sets: Vec<TemplateEvalSet>,
    pub deployment_checklist: Vec<String>,
    pub smoke_script: String,
    pub smoke_checks: Vec<TemplateSmokeCheck>,
    pub deployment_steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerPackageApplyPreview {
    pub package: CustomerPackageResp,
    pub tenant_code: String,
    pub applied_steps: Vec<CustomerProvisioningStep>,
    pub pending_operator_steps: Vec<CustomerProvisioningStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerPackageApplyResp {
    pub package: CustomerPackageResp,
    pub tenant_id: i64,
    pub tenant_code: String,
    pub applied_steps: Vec<CustomerProvisioningStep>,
    pub pending_operator_steps: Vec<CustomerProvisioningStep>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSmokeRunCommand {
    pub template_code: String,
    #[serde(default)]
    pub package_id: Option<String>,
    #[serde(default = "default_true")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSmokeRunResp {
    pub run_id: i64,
    pub template_code: String,
    pub package_id: Option<String>,
    pub smoke_script: String,
    pub status: String,
    pub dry_run: bool,
    pub total_checks: i32,
    pub passed_checks: i32,
    pub failed_checks: i32,
    pub checks: Vec<TemplateSmokeCheckRunResp>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSmokeCheckRunResp {
    pub code: String,
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: i64,
}

pub fn delivery_templates() -> Result<Vec<DeliveryTemplate>, AppError> {
    let mut templates = TEMPLATE_MANIFESTS
        .iter()
        .map(|manifest| {
            serde_json::from_str::<DeliveryTemplate>(manifest)
                .map_err(|err| AppError::bad_request(format!("交付模板格式错误: {err}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    for template in &templates {
        validate_template_manifest(template)?;
    }
    templates.sort_by_key(|template| (template.sort, template.code.clone()));
    Ok(templates)
}

pub fn list_delivery_templates(
    query: DeliveryTemplateQuery,
) -> Result<PageResult<DeliveryTemplate>, AppError> {
    let page = query.page_query();
    let category = query
        .category
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let filtered = delivery_templates()?
        .into_iter()
        .filter(|template| query.status.is_none_or(|status| template.status == status))
        .filter(|template| category.is_none_or(|value| template.category == value))
        .collect::<Vec<_>>();
    let total = filtered.len() as i64;
    let list = filtered
        .into_iter()
        .skip(page.offset() as usize)
        .take(page.limit() as usize)
        .collect();
    Ok(PageResult::new(list, total))
}

pub fn get_delivery_template(code: &str) -> Result<DeliveryTemplate, AppError> {
    let code = code.trim();
    delivery_templates()?
        .into_iter()
        .find(|template| template.code == code)
        .ok_or(AppError::NotFound)
}

pub fn build_customer_package(
    command: CustomerPackageCommand,
) -> Result<CustomerPackageResp, AppError> {
    let command = normalize_customer_package_command(command)?;
    let template = get_delivery_template(&command.template_code)?;
    let mut branding = template.branding.clone();
    if let Some(brand_name) = non_empty_owned(command.brand_name) {
        branding.brand_name = brand_name;
    }
    if let Some(primary_color) = non_empty_owned(command.primary_color) {
        branding.primary_color = primary_color;
    }
    if let Some(public_url) = non_empty_owned(command.public_url) {
        branding.public_url = public_url;
    }

    let tenant_config = CustomerTenantConfig {
        customer_name: command.customer_name,
        app_name: command.app_name,
        industry: non_empty_owned(command.industry).unwrap_or_else(|| template.category.clone()),
        template_code: template.code.clone(),
        frontend_entry: template.frontend_entry.clone(),
        frontend_app: template.frontend_app.clone(),
    };
    let package_id = format!(
        "pkg_{}_{}",
        template.code,
        slug_component(&tenant_config.customer_name)
    );
    let frontend_config = build_frontend_config(&template, &branding);
    let provisioning_plan = build_provisioning_plan(
        &package_id,
        &tenant_config,
        &branding,
        &frontend_config,
        &template,
    );
    let deployment_steps = build_deployment_steps(&tenant_config, &template);

    Ok(CustomerPackageResp {
        package_id,
        template: template.clone(),
        tenant_config,
        branding,
        frontend_config,
        provisioning_plan,
        roles: template.roles.clone(),
        menus: template.menus.clone(),
        frontend_pages: template.frontend_pages.clone(),
        prompts: template.prompts.clone(),
        skills: template.skills.clone(),
        connectors: template.connectors.clone(),
        plugins: template.plugins.clone(),
        triggers: template.triggers.clone(),
        eval_sets: template.eval_sets.clone(),
        deployment_checklist: template.deployment_checklist.clone(),
        smoke_script: template.smoke_script.clone(),
        smoke_checks: template.smoke_checks.clone(),
        deployment_steps,
    })
}

pub fn build_customer_package_apply_preview(
    command: CustomerPackageCommand,
) -> Result<CustomerPackageApplyPreview, AppError> {
    let package = build_customer_package(command)?;
    let tenant_code = package.provisioning_plan.tenant_code.clone();
    let mut applied_steps = Vec::new();
    let mut pending_operator_steps = Vec::new();

    for step in &package.provisioning_plan.steps {
        if matches!(
            step.code.as_str(),
            "tenant" | "roles" | "menus" | "frontend" | "capabilities" | "eval"
        ) {
            applied_steps.push(step.clone());
        } else {
            pending_operator_steps.push(step.clone());
        }
    }

    Ok(CustomerPackageApplyPreview {
        package,
        tenant_code,
        applied_steps,
        pending_operator_steps,
    })
}

pub async fn apply_customer_package(
    db: &PgPool,
    user_id: i64,
    command: CustomerPackageCommand,
) -> Result<CustomerPackageApplyResp, AppError> {
    let preview = build_customer_package_apply_preview(command)?;
    let now = Utc::now().naive_utc();
    let package = &preview.package;
    let record = CustomerPackageApplyRecord {
        package_id: package.package_id.clone(),
        tenant_code: preview.tenant_code.clone(),
        tenant_name: package.tenant_config.customer_name.clone(),
        plan_code: "m5_customer_package".to_owned(),
        template_code: package.tenant_config.template_code.clone(),
        customer_name: package.tenant_config.customer_name.clone(),
        app_name: package.tenant_config.app_name.clone(),
        package_payload: json!(package),
        provisioning_plan: json!(package.provisioning_plan),
        applied_steps: json!(preview.applied_steps),
        pending_operator_steps: json!(preview.pending_operator_steps),
        frontend_config: CustomerPackageFrontendConfigApplyRecord {
            app: package.frontend_config.app.clone(),
            entry: package.frontend_config.entry.clone(),
            entry_url: package.frontend_config.entry_url.clone(),
            default_path: package.frontend_config.default_page.path.clone(),
            branding: json!(package.frontend_config.branding),
            navigation: json!(package.frontend_config.navigation),
            allowed_roles: json!(package.frontend_config.allowed_roles),
        },
        roles: package
            .roles
            .iter()
            .map(|role| CustomerPackageRoleApplyRecord {
                code: role.code.clone(),
                name: role.name.clone(),
                permissions: role.permissions.clone(),
            })
            .collect(),
        menus: package
            .menus
            .iter()
            .map(|menu| CustomerPackageMenuApplyRecord {
                code: menu.code.clone(),
                title: menu.title.clone(),
                path: menu.path.clone(),
                permission: menu.permission.clone(),
            })
            .collect(),
        skills: package
            .skills
            .iter()
            .map(|skill| CustomerPackageSkillApplyRecord {
                code: skill.code.clone(),
                name: skill.name.clone(),
                description: skill.description.clone(),
            })
            .collect(),
        connectors: package
            .connectors
            .iter()
            .map(|connector| CustomerPackageConnectorApplyRecord {
                code: connector.code.clone(),
                name: connector.name.clone(),
                kind: connector.kind.clone(),
                auth_type: connector.auth_type.clone(),
            })
            .collect(),
        plugins: package
            .plugins
            .iter()
            .map(|plugin| CustomerPackagePluginApplyRecord {
                code: plugin.code.clone(),
                name: plugin.name.clone(),
                runtime: plugin.runtime.clone(),
            })
            .collect(),
        triggers: package
            .triggers
            .iter()
            .map(|trigger| CustomerPackageTriggerApplyRecord {
                code: trigger.code.clone(),
                name: trigger.name.clone(),
                source_type: trigger.source_type.clone(),
                target: trigger.target.clone(),
            })
            .collect(),
        eval_sets: package
            .eval_sets
            .iter()
            .map(|eval_set| CustomerPackageEvalSetApplyRecord {
                code: eval_set.code.clone(),
                name: eval_set.name.clone(),
                case_count: eval_set.case_count,
                metrics: eval_set.metrics.clone(),
            })
            .collect(),
        metadata: json!({
            "packageId": package.package_id,
            "templateCode": package.tenant_config.template_code,
            "industry": package.tenant_config.industry,
            "frontendApp": package.tenant_config.frontend_app,
            "frontendEntry": package.tenant_config.frontend_entry,
            "branding": package.branding,
        }),
        user_id,
        now,
    };
    let applied = AiTemplateRepository::new(db.clone())
        .apply_customer_package(&record)
        .await?;

    Ok(CustomerPackageApplyResp {
        package: preview.package,
        tenant_id: applied.tenant_id,
        tenant_code: applied.tenant_code,
        applied_steps: preview.applied_steps,
        pending_operator_steps: preview.pending_operator_steps,
    })
}

pub async fn run_template_smoke(
    db: &PgPool,
    tenant_id: i64,
    user_id: i64,
    command: TemplateSmokeRunCommand,
) -> Result<TemplateSmokeRunResp, AppError> {
    let command = normalize_template_smoke_run_command(command)?;
    let template = get_delivery_template(&command.template_code)?;
    let run_id = next_id();
    let checks = if command.dry_run {
        template
            .smoke_checks
            .iter()
            .map(planned_smoke_check)
            .collect::<Vec<_>>()
    } else {
        let mut checks = Vec::with_capacity(template.smoke_checks.len());
        for check in &template.smoke_checks {
            checks.push(execute_smoke_check(check).await);
        }
        checks
    };
    let total_checks = checks.len() as i32;
    let passed_checks = checks
        .iter()
        .filter(|check| check.status == "passed")
        .count() as i32;
    let failed_checks = checks
        .iter()
        .filter(|check| check.status == "failed")
        .count() as i32;
    let status = if command.dry_run {
        "planned"
    } else if failed_checks == 0 {
        "passed"
    } else {
        "failed"
    }
    .to_owned();
    let now = Utc::now().naive_utc();
    let repo = AiTemplateRepository::new(db.clone());
    repo.create_template_smoke_run(&TemplateSmokeRunSaveRecord {
        id: run_id,
        tenant_id,
        template_code: template.code.clone(),
        package_id: command.package_id.clone(),
        smoke_script: template.smoke_script.clone(),
        status: status.clone(),
        dry_run: command.dry_run,
        total_checks,
        passed_checks,
        failed_checks,
        result_payload: json!({ "checks": checks }),
        metadata: json!({
            "source": "template_smoke_runner",
            "templateName": template.name,
        }),
        user_id,
        now,
    })
    .await?;
    for check in &checks {
        repo.create_template_smoke_result(&TemplateSmokeResultSaveRecord {
            id: next_id(),
            tenant_id,
            run_id,
            check_code: check.code.clone(),
            name: check.name.clone(),
            workdir: check.workdir.clone(),
            command: check.command.clone(),
            status: check.status.clone(),
            exit_code: check.exit_code,
            stdout: empty_to_none(&check.stdout),
            stderr: empty_to_none(&check.stderr),
            duration_ms: check.duration_ms,
            user_id,
            now,
        })
        .await?;
    }

    Ok(TemplateSmokeRunResp {
        run_id,
        template_code: template.code,
        package_id: command.package_id,
        smoke_script: template.smoke_script,
        status,
        dry_run: command.dry_run,
        total_checks,
        passed_checks,
        failed_checks,
        checks,
    })
}

fn build_provisioning_plan(
    package_id: &str,
    tenant_config: &CustomerTenantConfig,
    branding: &TemplateBranding,
    frontend_config: &CustomerFrontendConfig,
    template: &DeliveryTemplate,
) -> CustomerProvisioningPlan {
    let tenant_code = tenant_code_component(&tenant_config.customer_name);
    let eval_set_codes = template
        .eval_sets
        .iter()
        .map(|item| item.code.clone())
        .collect::<Vec<_>>();
    let smoke_check_codes = template
        .smoke_checks
        .iter()
        .map(|item| item.code.clone())
        .collect::<Vec<_>>();

    CustomerProvisioningPlan {
        plan_id: format!("prov_{package_id}"),
        mode: "operator_applied".to_owned(),
        idempotency_key: format!("{}:{tenant_code}", template.code),
        tenant_code: tenant_code.clone(),
        steps: vec![
            CustomerProvisioningStep {
                code: "tenant".to_owned(),
                title: "Create or update tenant".to_owned(),
                target: "sys_tenant".to_owned(),
                operation: "upsert".to_owned(),
                payload: json!({
                    "tenantCode": tenant_code,
                    "customerName": tenant_config.customer_name,
                    "appName": tenant_config.app_name,
                    "industry": tenant_config.industry,
                    "templateCode": tenant_config.template_code,
                    "planCode": "m5_customer_package",
                    "metadata": {
                        "packageId": package_id,
                        "template": template.code,
                        "frontendApp": tenant_config.frontend_app,
                        "frontendEntry": tenant_config.frontend_entry
                    }
                }),
            },
            CustomerProvisioningStep {
                code: "roles".to_owned(),
                title: "Create roles and bind permissions".to_owned(),
                target: "sys_role".to_owned(),
                operation: "upsert_and_bind".to_owned(),
                payload: json!({
                    "roles": template.roles,
                    "roleMenuBindings": build_role_menu_bindings(template),
                    "tenantRoleBinding": true
                }),
            },
            CustomerProvisioningStep {
                code: "menus".to_owned(),
                title: "Create app menus".to_owned(),
                target: "sys_menu".to_owned(),
                operation: "upsert".to_owned(),
                payload: json!({
                    "menus": template.menus,
                    "frontendPages": template.frontend_pages
                }),
            },
            CustomerProvisioningStep {
                code: "frontend".to_owned(),
                title: "Apply branding and frontend publish config".to_owned(),
                target: "frontend_config".to_owned(),
                operation: "upsert".to_owned(),
                payload: json!({
                    "branding": branding,
                    "frontendConfig": frontend_config
                }),
            },
            CustomerProvisioningStep {
                code: "capabilities".to_owned(),
                title: "Enable template skills, connectors, plugins, and triggers".to_owned(),
                target: "ai_capability_registry".to_owned(),
                operation: "enable".to_owned(),
                payload: json!({
                    "skills": template.skills,
                    "connectors": template.connectors,
                    "plugins": template.plugins,
                    "triggers": template.triggers,
                    "credentialPolicy": "bind_via_secret_control_plane"
                }),
            },
            CustomerProvisioningStep {
                code: "eval".to_owned(),
                title: "Select default eval sets".to_owned(),
                target: "ai_eval_dataset".to_owned(),
                operation: "upsert_and_select".to_owned(),
                payload: json!({
                    "evalSets": template.eval_sets,
                    "evalSetCodes": eval_set_codes
                }),
            },
            CustomerProvisioningStep {
                code: "smoke".to_owned(),
                title: "Run delivery smoke checks".to_owned(),
                target: "delivery_smoke".to_owned(),
                operation: "run".to_owned(),
                payload: json!({
                    "smokeScript": template.smoke_script,
                    "smokeChecks": template.smoke_checks,
                    "smokeCheckCodes": smoke_check_codes
                }),
            },
        ],
    }
}

fn build_role_menu_bindings(template: &DeliveryTemplate) -> Vec<Value> {
    template
        .roles
        .iter()
        .map(|role| {
            let menu_codes = template
                .menus
                .iter()
                .filter(|menu| role.permissions.contains(&menu.permission))
                .map(|menu| menu.code.clone())
                .collect::<Vec<_>>();
            json!({
                "roleCode": role.code,
                "menuCodes": menu_codes
            })
        })
        .collect()
}

fn validate_template_manifest(template: &DeliveryTemplate) -> Result<(), AppError> {
    if template.code.trim().is_empty()
        || template.name.trim().is_empty()
        || template.category.trim().is_empty()
        || template.frontend_entry.trim().is_empty()
        || template.frontend_app.trim().is_empty()
        || template.smoke_script.trim().is_empty()
    {
        return Err(AppError::bad_request("交付模板基础字段不能为空"));
    }
    if template.roles.is_empty()
        || template.menus.is_empty()
        || template.eval_sets.is_empty()
        || template.branding.brand_name.trim().is_empty()
        || template.deployment_checklist.is_empty()
        || template.frontend_pages.is_empty()
        || template.smoke_checks.is_empty()
    {
        return Err(AppError::bad_request(format!(
            "交付模板缺少必要交付段: {}",
            template.code
        )));
    }
    Ok(())
}

fn normalize_customer_package_command(
    mut command: CustomerPackageCommand,
) -> Result<CustomerPackageCommand, AppError> {
    command.template_code = command.template_code.trim().to_owned();
    command.customer_name = command.customer_name.trim().to_owned();
    command.app_name = command.app_name.trim().to_owned();
    if command.template_code.is_empty() {
        return Err(AppError::bad_request("模板编码不能为空"));
    }
    if command.customer_name.is_empty() {
        return Err(AppError::bad_request("客户名称不能为空"));
    }
    if command.app_name.is_empty() {
        return Err(AppError::bad_request("应用名称不能为空"));
    }
    ensure_max_chars("模板编码", &command.template_code, 128)?;
    ensure_max_chars("客户名称", &command.customer_name, 128)?;
    ensure_max_chars("应用名称", &command.app_name, 128)?;
    Ok(command)
}

fn normalize_template_smoke_run_command(
    mut command: TemplateSmokeRunCommand,
) -> Result<TemplateSmokeRunCommand, AppError> {
    command.template_code = command.template_code.trim().to_owned();
    command.package_id = non_empty_owned(command.package_id);
    if command.template_code.is_empty() {
        return Err(AppError::bad_request("模板编码不能为空"));
    }
    ensure_max_chars("模板编码", &command.template_code, 128)?;
    if let Some(package_id) = &command.package_id {
        ensure_max_chars("交付包 ID", package_id, 320)?;
    }
    Ok(command)
}

fn planned_smoke_check(check: &TemplateSmokeCheck) -> TemplateSmokeCheckRunResp {
    TemplateSmokeCheckRunResp {
        code: check.code.clone(),
        name: check.name.clone(),
        workdir: check.workdir.clone(),
        command: check.command.clone(),
        status: "planned".to_owned(),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: 0,
    }
}

async fn execute_smoke_check(check: &TemplateSmokeCheck) -> TemplateSmokeCheckRunResp {
    let started_at = Instant::now();
    let workdir = match safe_smoke_workdir(&check.workdir) {
        Ok(path) => path,
        Err(err) => {
            return TemplateSmokeCheckRunResp {
                status: "failed".to_owned(),
                stderr: err.to_string(),
                duration_ms: elapsed_ms(started_at),
                ..planned_smoke_check(check)
            };
        }
    };
    let output = timeout(
        Duration::from_secs(DEFAULT_SMOKE_TIMEOUT_SECS),
        Command::new("bash")
            .arg("-lc")
            .arg(&check.command)
            .current_dir(workdir)
            .output(),
    )
    .await;

    match output {
        Ok(Ok(output)) => TemplateSmokeCheckRunResp {
            status: if output.status.success() {
                "passed".to_owned()
            } else {
                "failed".to_owned()
            },
            exit_code: output.status.code(),
            stdout: bounded_process_output(output.stdout),
            stderr: bounded_process_output(output.stderr),
            duration_ms: elapsed_ms(started_at),
            ..planned_smoke_check(check)
        },
        Ok(Err(err)) => TemplateSmokeCheckRunResp {
            status: "failed".to_owned(),
            stderr: err.to_string(),
            duration_ms: elapsed_ms(started_at),
            ..planned_smoke_check(check)
        },
        Err(_) => TemplateSmokeCheckRunResp {
            status: "failed".to_owned(),
            stderr: format!("smoke check timed out after {DEFAULT_SMOKE_TIMEOUT_SECS}s"),
            duration_ms: elapsed_ms(started_at),
            ..planned_smoke_check(check)
        },
    }
}

fn safe_smoke_workdir(workdir: &str) -> Result<PathBuf, AppError> {
    let root = repo_root();
    let candidate = root.join(workdir);
    let canonical_root = root.canonicalize()?;
    let canonical_candidate = candidate.canonicalize()?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(AppError::bad_request("Smoke 工作目录必须位于仓库内"));
    }
    Ok(canonical_candidate)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../")
}

fn bounded_process_output(output: Vec<u8>) -> String {
    const MAX_OUTPUT_CHARS: usize = 8000;
    let output = String::from_utf8_lossy(&output);
    output.chars().take(MAX_OUTPUT_CHARS).collect()
}

fn elapsed_ms(started_at: Instant) -> i64 {
    started_at
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn empty_to_none(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

fn build_deployment_steps(
    tenant_config: &CustomerTenantConfig,
    template: &DeliveryTemplate,
) -> Vec<String> {
    let mut steps = vec![
        format!(
            "Create tenant config for {} using {}",
            tenant_config.customer_name, template.code
        ),
        format!("Initialize roles and menus for {}", tenant_config.app_name),
        format!(
            "Apply branding and frontend entry {}",
            template.frontend_entry
        ),
    ];
    steps.extend(template.deployment_checklist.iter().cloned());
    steps.extend(template.smoke_checks.iter().map(|check| {
        format!(
            "Run smoke check {} in {}: {}",
            check.name, check.workdir, check.command
        )
    }));
    steps
}

fn build_frontend_config(
    template: &DeliveryTemplate,
    branding: &TemplateBranding,
) -> CustomerFrontendConfig {
    let frontend_permissions = template
        .frontend_pages
        .iter()
        .map(|page| page.permission.as_str())
        .collect::<HashSet<_>>();
    let allowed_roles = template
        .roles
        .iter()
        .filter(|role| {
            role.permissions
                .iter()
                .any(|permission| frontend_permissions.contains(permission.as_str()))
        })
        .cloned()
        .collect();

    CustomerFrontendConfig {
        app: template.frontend_app.clone(),
        entry: template.frontend_entry.clone(),
        entry_url: branding.public_url.clone(),
        branding: branding.clone(),
        default_page: template.frontend_pages[0].clone(),
        navigation: template.frontend_pages.clone(),
        allowed_roles,
    }
}

fn non_empty_owned(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
}

fn slug_component(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "customer".to_owned()
    } else {
        slug
    }
}

fn tenant_code_component(value: &str) -> String {
    let truncated = slug_component(value)
        .chars()
        .take(64)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned();
    if truncated.is_empty() {
        "customer".to_owned()
    } else {
        truncated
    }
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_template_size() -> u64 {
    DEFAULT_TEMPLATE_PAGE_SIZE
}

fn default_enabled_status() -> Option<i16> {
    Some(ENABLED_STATUS)
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template_readme(code: &str) -> &'static str {
        match code {
            "llm_chat" => include_str!("../../../../templates/llm-chat/README.md"),
            "knowledge_base_chat" => {
                include_str!("../../../../templates/knowledge-base-chat/README.md")
            }
            "agent_workspace" => include_str!("../../../../templates/agent-workspace/README.md"),
            "customer_service_agent" => {
                include_str!("../../../../templates/customer-service-agent/README.md")
            }
            "training_app" => include_str!("../../../../templates/training-app/README.md"),
            _ => "",
        }
    }

    fn template_manifest(code: &str) -> &'static str {
        match code {
            "llm_chat" => include_str!("../../../../templates/llm-chat/template.json"),
            "knowledge_base_chat" => {
                include_str!("../../../../templates/knowledge-base-chat/template.json")
            }
            "agent_workspace" => {
                include_str!("../../../../templates/agent-workspace/template.json")
            }
            "customer_service_agent" => {
                include_str!("../../../../templates/customer-service-agent/template.json")
            }
            "training_app" => include_str!("../../../../templates/training-app/template.json"),
            _ => "",
        }
    }

    fn template_dir(code: &str) -> &'static str {
        match code {
            "llm_chat" => "llm-chat",
            "knowledge_base_chat" => "knowledge-base-chat",
            "agent_workspace" => "agent-workspace",
            "customer_service_agent" => "customer-service-agent",
            "training_app" => "training-app",
            _ => "",
        }
    }

    #[test]
    fn delivery_templates_include_all_m5_defaults() {
        let templates = delivery_templates().unwrap();
        let codes = templates
            .iter()
            .map(|template| template.code.as_str())
            .collect::<Vec<_>>();

        assert_eq!(templates.len(), 5);
        assert!(codes.contains(&"llm_chat"));
        assert!(codes.contains(&"knowledge_base_chat"));
        assert!(codes.contains(&"agent_workspace"));
        assert!(codes.contains(&"customer_service_agent"));
        assert!(codes.contains(&"training_app"));
    }

    #[test]
    fn delivery_template_manifest_requires_roles_menus_branding_and_eval_sets() {
        let templates = delivery_templates().unwrap();

        for template in templates {
            assert!(
                !template.roles.is_empty(),
                "{} missing roles",
                template.code
            );
            assert!(
                !template.menus.is_empty(),
                "{} missing menus",
                template.code
            );
            assert!(
                !template.eval_sets.is_empty(),
                "{} missing eval sets",
                template.code
            );
            assert!(
                !template.branding.brand_name.trim().is_empty(),
                "{} missing brand",
                template.code
            );
        }
    }

    #[test]
    fn delivery_template_manifest_registers_training_customer_frontend_metadata() {
        let template = get_delivery_template("training_app").unwrap();
        let page_codes = template
            .frontend_pages
            .iter()
            .map(|page| page.code.as_str())
            .collect::<Vec<_>>();

        assert_eq!(template.frontend_app, "training-web");
        assert_eq!(
            page_codes,
            vec!["learn", "ask", "quiz", "records", "notifications"]
        );
        assert!(template
            .smoke_checks
            .iter()
            .any(|check| { check.workdir == "apps/training-web" && check.command == "pnpm test" }));
        let training_admin = template
            .roles
            .iter()
            .find(|role| role.code == "training_admin")
            .unwrap();
        for permission in ["ai:eval:list", "ai:eval:run", "ai:eval:report"] {
            assert!(training_admin.permissions.contains(&permission.to_owned()));
        }
    }

    #[test]
    fn delivery_template_training_roles_cover_customer_workbench_permissions() {
        let template = get_delivery_template("training_app").unwrap();
        let training_admin = template
            .roles
            .iter()
            .find(|role| role.code == "training_admin")
            .unwrap();
        let learner = template
            .roles
            .iter()
            .find(|role| role.code == "learner")
            .unwrap();

        for permission in [
            "app:training:learn",
            "app:training:ask",
            "app:training:quiz",
            "ai:knowledge:list",
            "ai:knowledge:ask",
            "ai:knowledge:document:create",
            "ai:agent:run",
            "ai:eval:list",
            "ai:eval:run",
            "ai:eval:report",
        ] {
            assert!(
                training_admin.permissions.contains(&permission.to_owned()),
                "training_admin missing {permission}"
            );
        }

        for permission in [
            "app:training:learn",
            "app:training:ask",
            "app:training:quiz",
            "ai:knowledge:list",
            "ai:knowledge:ask",
            "ai:agent:run",
        ] {
            assert!(
                learner.permissions.contains(&permission.to_owned()),
                "learner missing {permission}"
            );
        }
    }

    #[test]
    fn delivery_template_manifest_registers_chat_customer_frontend_metadata() {
        let template = get_delivery_template("knowledge_base_chat").unwrap();
        let page_codes = template
            .frontend_pages
            .iter()
            .map(|page| page.code.as_str())
            .collect::<Vec<_>>();

        assert_eq!(template.frontend_app, "chat-web");
        assert_eq!(page_codes, vec!["ask", "sources", "share"]);
        let share_page = template
            .frontend_pages
            .iter()
            .find(|page| page.code == "share")
            .unwrap();
        assert_eq!(share_page.path, "/share/[token]");
        assert_eq!(share_page.permission, "app:knowledge:ask");
        assert!(template
            .smoke_checks
            .iter()
            .any(|check| { check.workdir == "apps/chat-web" && check.command == "pnpm test" }));
    }

    #[test]
    fn delivery_template_knowledge_roles_cover_chat_web_permissions() {
        let template = get_delivery_template("knowledge_base_chat").unwrap();
        let knowledge_admin = template
            .roles
            .iter()
            .find(|role| role.code == "knowledge_admin")
            .unwrap();
        let knowledge_user = template
            .roles
            .iter()
            .find(|role| role.code == "knowledge_user")
            .unwrap();

        assert!(!knowledge_admin
            .permissions
            .contains(&"ai:rag:ask".to_owned()));

        for permission in [
            "app:knowledge:ask",
            "ai:knowledge:list",
            "ai:knowledge:create",
            "ai:knowledge:document:list",
            "ai:knowledge:document:create",
            "ai:knowledge:ask",
            "ai:chatFlow:list",
            "ai:chatFlow:create",
            "ai:chatFlow:message",
        ] {
            assert!(
                knowledge_admin.permissions.contains(&permission.to_owned()),
                "knowledge_admin missing {permission}"
            );
        }

        for permission in [
            "app:knowledge:ask",
            "ai:knowledge:list",
            "ai:knowledge:ask",
            "ai:chatFlow:list",
            "ai:chatFlow:create",
            "ai:chatFlow:message",
        ] {
            assert!(
                knowledge_user.permissions.contains(&permission.to_owned()),
                "knowledge_user missing {permission}"
            );
        }
    }

    #[test]
    fn delivery_template_manifest_registers_llm_chat_runtime_permission() {
        let template = get_delivery_template("llm_chat").unwrap();
        let chat_user = template
            .roles
            .iter()
            .find(|role| role.code == "chat_user")
            .unwrap();
        let page_codes = template
            .frontend_pages
            .iter()
            .map(|page| page.code.as_str())
            .collect::<Vec<_>>();
        let share_page = template
            .frontend_pages
            .iter()
            .find(|page| page.code == "share")
            .unwrap();

        assert_eq!(template.frontend_app, "chat-web");
        assert_eq!(page_codes, vec!["chat", "history", "share"]);
        assert_eq!(share_page.path, "/share/[token]");
        assert_eq!(share_page.permission, "app:chat:use");
        assert!(chat_user.permissions.contains(&"app:chat:use".to_owned()));
        assert!(chat_user.permissions.contains(&"ai:model:chat".to_owned()));
        assert!(template.smoke_checks.iter().any(|check| {
            check.workdir == "backend" && check.command.contains("model_service")
        }));
    }

    #[test]
    fn delivery_template_manifest_registers_agent_customer_frontend_metadata() {
        let template = get_delivery_template("agent_workspace").unwrap();
        let page_codes = template
            .frontend_pages
            .iter()
            .map(|page| page.code.as_str())
            .collect::<Vec<_>>();

        assert_eq!(template.frontend_app, "agent-workspace");
        assert_eq!(page_codes, vec!["workspace", "approvals", "traces"]);
        assert!(template.smoke_checks.iter().any(|check| {
            check.workdir == "apps/agent-workspace" && check.command == "pnpm test"
        }));
    }

    #[test]
    fn delivery_template_manifest_registers_customer_service_agent_metadata() {
        let template = get_delivery_template("customer_service_agent").unwrap();
        let page_codes = template
            .frontend_pages
            .iter()
            .map(|page| page.code.as_str())
            .collect::<Vec<_>>();
        let operator = template
            .roles
            .iter()
            .find(|role| role.code == "customer_service_operator")
            .unwrap();
        let eval_set = template
            .eval_sets
            .iter()
            .find(|eval_set| eval_set.code == "customer-service-agent-regression")
            .unwrap();

        assert_eq!(template.frontend_app, "customer-service-agent");
        assert_eq!(page_codes, vec!["console", "runs", "knowledge"]);
        assert!(operator
            .permissions
            .contains(&"ai:customer-service:agent:run".to_owned()));
        assert!(operator
            .permissions
            .contains(&"ai:customer-service:read".to_owned()));
        assert!(eval_set.metrics.contains(&"citation_accuracy".to_owned()));
        assert!(template.smoke_checks.iter().any(|check| {
            check.workdir == "backend" && check.command.contains("customer_service_")
        }));
    }

    #[test]
    fn delivery_template_smoke_checks_cover_frontend_app() {
        for template in delivery_templates().unwrap() {
            assert!(
                template
                    .smoke_checks
                    .iter()
                    .any(|check| check.workdir == template.frontend_entry),
                "{} smoke checks must cover frontend entry {}",
                template.code,
                template.frontend_entry
            );
        }
    }

    #[test]
    fn delivery_template_menus_cover_customer_frontend_pages() {
        for template in delivery_templates().unwrap() {
            for page in &template.frontend_pages {
                if page.code == "share" {
                    continue;
                }
                let menu = template
                    .menus
                    .iter()
                    .find(|menu| menu.path == page.path)
                    .unwrap_or_else(|| {
                        panic!(
                            "{} menu paths must include frontend page {} ({})",
                            template.code, page.code, page.path
                        )
                    });
                assert_eq!(
                    menu.permission, page.permission,
                    "{} menu paths must include frontend page {} ({})",
                    template.code, page.code, page.path
                );
            }
        }
    }

    #[test]
    fn delivery_template_plugins_resolve_to_valid_builtin_manifests() {
        for template in delivery_templates().unwrap() {
            for plugin in &template.plugins {
                let manifest =
                    novex_plugin::builtin_plugin_manifest(&plugin.code).unwrap_or_else(|| {
                        panic!(
                            "{} plugin {} missing builtin manifest",
                            template.code, plugin.code
                        )
                    });

                assert_eq!(manifest.code, plugin.code);
                assert_eq!(manifest.runtime.as_str(), plugin.runtime);
                novex_plugin::validate_plugin_manifest(&manifest).unwrap_or_else(|err| {
                    panic!(
                        "{} plugin {} manifest must be valid: {:?}",
                        template.code, plugin.code, err
                    )
                });
            }
        }
    }

    #[test]
    fn delivery_template_readmes_list_frontend_pages_and_smoke_checks() {
        for template in delivery_templates().unwrap() {
            let readme = template_readme(&template.code);
            assert!(
                readme.contains("Frontend pages"),
                "{} README missing frontend page section",
                template.code
            );
            assert!(
                readme.contains("Smoke checks"),
                "{} README missing smoke check section",
                template.code
            );
            for page in &template.frontend_pages {
                assert!(
                    readme.contains(&page.path) && readme.contains(&page.permission),
                    "{} README missing page {} ({})",
                    template.code,
                    page.path,
                    page.permission
                );
            }
            for check in &template.smoke_checks {
                assert!(
                    readme.contains(&check.workdir) && readme.contains(&check.command),
                    "{} README missing smoke check {}",
                    template.code,
                    check.code
                );
            }
        }
    }

    #[test]
    fn delivery_template_manifests_reference_smoke_scripts_that_cover_checks() {
        for template in delivery_templates().unwrap() {
            let manifest = template_manifest(&template.code);
            let script_path = format!("templates/{}/smoke.sh", template_dir(&template.code));
            let absolute_script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../")
                .join(&script_path);
            let script = std::fs::read_to_string(&absolute_script_path)
                .unwrap_or_else(|err| panic!("{} smoke script missing: {err}", template.code));

            assert!(
                manifest.contains("\"smokeScript\""),
                "{} manifest missing smokeScript",
                template.code
            );
            assert!(
                manifest.contains(&script_path),
                "{} manifest does not reference {}",
                template.code,
                script_path
            );
            for check in &template.smoke_checks {
                assert!(
                    script.contains(&check.workdir) && script.contains(&check.command),
                    "{} script missing smoke check {}",
                    template.code,
                    check.code
                );
            }
        }
    }

    #[test]
    fn delivery_manual_documents_public_link_share_publication_flow() {
        let manual = include_str!("../../../../docs/delivery/novex-customer-delivery.md");

        for required in [
            "Public Link",
            "/share/[token]",
            "ai:integration:create",
            "provisioningPlan",
            "QPS",
            "quota",
            "publish/share",
        ] {
            assert!(
                manual.contains(required),
                "delivery manual missing {required}"
            );
        }
    }

    #[test]
    fn customer_package_generation_merges_customer_branding() {
        let package = build_customer_package(CustomerPackageCommand {
            template_code: "training_app".to_owned(),
            customer_name: "Acme".to_owned(),
            app_name: "Acme Training".to_owned(),
            industry: Some("training".to_owned()),
            brand_name: Some("Acme Academy".to_owned()),
            primary_color: Some("#2563eb".to_owned()),
            public_url: Some("https://training.example.com".to_owned()),
        })
        .unwrap();

        assert_eq!(package.template.code, "training_app");
        assert_eq!(package.tenant_config.customer_name, "Acme");
        assert_eq!(package.branding.brand_name, "Acme Academy");
        assert_eq!(package.branding.primary_color, "#2563eb");
        assert!(package
            .eval_sets
            .iter()
            .any(|item| item.code == "training_regression"));
        assert!(package
            .deployment_steps
            .iter()
            .any(|step| step.contains("eval")));
        let package_json = serde_json::to_value(&package).unwrap();
        assert_eq!(
            package_json["smokeScript"],
            "templates/training-app/smoke.sh"
        );
    }

    #[test]
    fn customer_package_exposes_frontend_publish_config() {
        let package = build_customer_package(CustomerPackageCommand {
            template_code: "training_app".to_owned(),
            customer_name: "Acme".to_owned(),
            app_name: "Acme Training".to_owned(),
            industry: Some("training".to_owned()),
            brand_name: Some("Acme Academy".to_owned()),
            primary_color: Some("#2563eb".to_owned()),
            public_url: Some("https://training.example.com".to_owned()),
        })
        .unwrap();

        assert_eq!(package.frontend_config.app, "training-web");
        assert_eq!(package.frontend_config.entry, "apps/training-web");
        assert_eq!(
            package.frontend_config.entry_url,
            "https://training.example.com"
        );
        assert_eq!(package.frontend_config.default_page.path, "/");
        assert_eq!(package.frontend_config.navigation.len(), 5);
        assert!(package
            .frontend_config
            .allowed_roles
            .iter()
            .any(|role| role.code == "learner"));
        assert!(package
            .frontend_config
            .allowed_roles
            .iter()
            .any(|role| role.code == "training_admin"));

        let package_json = serde_json::to_value(&package).unwrap();
        assert_eq!(
            package_json["frontendConfig"]["branding"]["brandName"],
            "Acme Academy"
        );
    }

    #[test]
    fn customer_package_exposes_machine_readable_provisioning_plan() {
        let package = build_customer_package(CustomerPackageCommand {
            template_code: "training_app".to_owned(),
            customer_name: "Acme".to_owned(),
            app_name: "Acme Training".to_owned(),
            industry: Some("training".to_owned()),
            brand_name: Some("Acme Academy".to_owned()),
            primary_color: Some("#2563eb".to_owned()),
            public_url: Some("https://training.example.com".to_owned()),
        })
        .unwrap();

        assert_eq!(
            package.provisioning_plan.plan_id,
            "prov_pkg_training_app_acme"
        );
        assert_eq!(package.provisioning_plan.mode, "operator_applied");
        assert_eq!(package.provisioning_plan.tenant_code, "acme");
        assert_eq!(
            package.provisioning_plan.idempotency_key,
            "training_app:acme"
        );

        let targets = package
            .provisioning_plan
            .steps
            .iter()
            .map(|step| step.target.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            targets,
            vec![
                "sys_tenant",
                "sys_role",
                "sys_menu",
                "frontend_config",
                "ai_capability_registry",
                "ai_eval_dataset",
                "delivery_smoke"
            ]
        );

        let role_step = package
            .provisioning_plan
            .steps
            .iter()
            .find(|step| step.code == "roles")
            .unwrap();
        assert_eq!(role_step.operation, "upsert_and_bind");
        assert_eq!(role_step.payload["roles"][0]["code"], "training_admin");
        assert_eq!(
            role_step.payload["roles"][0]["permissions"][0],
            "app:training:learn"
        );

        let capability_step = package
            .provisioning_plan
            .steps
            .iter()
            .find(|step| step.code == "capabilities")
            .unwrap();
        assert!(capability_step.payload["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "training_quiz"));
        assert!(capability_step.payload["connectors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "feishu.message"));

        let smoke_step = package
            .provisioning_plan
            .steps
            .iter()
            .find(|step| step.code == "smoke")
            .unwrap();
        assert_eq!(
            smoke_step.payload["smokeScript"],
            "templates/training-app/smoke.sh"
        );

        let package_json = serde_json::to_value(&package).unwrap();
        assert_eq!(
            package_json["provisioningPlan"]["steps"][0]["operation"],
            "upsert"
        );
    }

    #[test]
    fn customer_package_apply_marks_frontend_config_and_remaining_database_steps_as_applied() {
        let apply = build_customer_package_apply_preview(CustomerPackageCommand {
            template_code: "training_app".to_owned(),
            customer_name: "Acme".to_owned(),
            app_name: "Acme Training".to_owned(),
            industry: Some("training".to_owned()),
            brand_name: Some("Acme Academy".to_owned()),
            primary_color: Some("#2563eb".to_owned()),
            public_url: Some("https://training.example.com".to_owned()),
        })
        .unwrap();

        assert_eq!(apply.package.package_id, "pkg_training_app_acme");
        assert_eq!(apply.tenant_code, "acme");
        assert_eq!(
            apply
                .applied_steps
                .iter()
                .map(|step| step.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "tenant",
                "roles",
                "menus",
                "frontend",
                "capabilities",
                "eval"
            ]
        );
        assert_eq!(
            apply
                .pending_operator_steps
                .iter()
                .map(|step| step.code.as_str())
                .collect::<Vec<_>>(),
            vec!["smoke"]
        );

        let apply_json = serde_json::to_value(&apply).unwrap();
        assert_eq!(apply_json["appliedSteps"][0]["target"], "sys_tenant");
        assert_eq!(apply_json["appliedSteps"][1]["target"], "sys_role");
        assert_eq!(apply_json["appliedSteps"][2]["target"], "sys_menu");
        assert_eq!(apply_json["appliedSteps"][3]["target"], "frontend_config");
        assert_eq!(
            apply_json["appliedSteps"][4]["target"],
            "ai_capability_registry"
        );
        assert_eq!(apply_json["appliedSteps"][5]["target"], "ai_eval_dataset");
        assert_eq!(
            apply_json["pendingOperatorSteps"][0]["target"],
            "delivery_smoke"
        );
    }

    #[test]
    fn customer_package_apply_repository_uses_idempotent_tenant_role_menu_and_snapshot_upserts() {
        let repository = include_str!("../../infrastructure/persistence/ai_template_repository.rs");

        for needle in [
            "pub async fn apply_customer_package",
            "INSERT INTO sys_tenant",
            "ON CONFLICT (code) DO UPDATE",
            "RETURNING id, code",
            "INSERT INTO sys_role",
            "INSERT INTO sys_tenant_role",
            "ON CONFLICT (tenant_id, role_id) DO UPDATE",
            "INSERT INTO sys_menu",
            "ON CONFLICT (title, parent_id) DO UPDATE",
            "INSERT INTO sys_role_menu",
            "WHERE permission = $1",
            "INSERT INTO ai_customer_frontend_config",
            "ON CONFLICT (package_id) DO UPDATE",
            "INSERT INTO ai_skill",
            "ON CONFLICT (tenant_id, code) DO UPDATE",
            "INSERT INTO ai_tool",
            "INSERT INTO ai_connector",
            "INSERT INTO ai_plugin",
            "INSERT INTO ai_plugin_version",
            "INSERT INTO ai_plugin_capability",
            "INSERT INTO ai_plugin_installation",
            "INSERT INTO ai_trigger",
            "INSERT INTO ai_eval_dataset",
            "INSERT INTO ai_eval_case",
            "ON CONFLICT (dataset_id, case_code) DO UPDATE",
            "INSERT INTO ai_customer_package",
            "ON CONFLICT (package_id) DO UPDATE",
            "provisioning_plan",
            "pending_operator_steps",
        ] {
            assert!(
                repository.contains(needle),
                "{needle} missing from repository"
            );
        }
    }

    #[test]
    fn customer_frontend_config_migration_defines_publish_config_snapshot_contract() {
        let migration_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/202606060014_create_ai_customer_frontend_config.sql");
        let migration = std::fs::read_to_string(&migration_path)
            .unwrap_or_else(|err| panic!("frontend config migration missing: {err}"));

        for required in [
            "CREATE TABLE IF NOT EXISTS ai_customer_frontend_config",
            "package_id",
            "frontend_entry",
            "entry_url",
            "branding",
            "navigation",
            "allowed_roles",
            "uk_ai_customer_frontend_config_package",
        ] {
            assert!(migration.contains(required), "missing {required}");
        }
    }

    #[test]
    fn template_smoke_run_migration_defines_run_and_result_contract() {
        let migration_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/202606060015_create_ai_template_smoke_run.sql");
        let migration = std::fs::read_to_string(&migration_path)
            .unwrap_or_else(|err| panic!("template smoke migration missing: {err}"));

        for required in [
            "CREATE TABLE IF NOT EXISTS ai_template_smoke_run",
            "CREATE TABLE IF NOT EXISTS ai_template_smoke_result",
            "template_code",
            "smoke_script",
            "dry_run",
            "passed_checks",
            "failed_checks",
            "duration_ms",
            "idx_ai_template_smoke_run_tenant_template",
        ] {
            assert!(migration.contains(required), "missing {required}");
        }
    }

    #[test]
    fn template_smoke_runner_uses_manifest_checks_and_shell_execution_boundary() {
        let source = include_str!("template_service.rs");
        let run_fn = ["pub async fn ", "run_template_smoke("].concat();
        let command = ["Command::", "new(\"bash\")"].concat();
        let shell_flag = [".arg(\"", "-lc\")"].concat();
        let persist_run = ["create_template_", "smoke_run"].concat();
        let persist_result = ["create_template_", "smoke_result"].concat();

        for required in [
            run_fn.as_str(),
            command.as_str(),
            "template.smoke_checks",
            shell_flag.as_str(),
            "safe_smoke_workdir",
            persist_run.as_str(),
            persist_result.as_str(),
        ] {
            assert!(source.contains(required), "missing {required}");
        }
    }
}
