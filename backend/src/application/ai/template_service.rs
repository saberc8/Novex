use serde::{Deserialize, Serialize};

use crate::{
    application::system::ensure_max_chars,
    shared::{
        error::AppError,
        pagination::{PageQuery, PageResult, DEFAULT_PAGE},
    },
};

const DEFAULT_TEMPLATE_PAGE_SIZE: u64 = 20;
const ENABLED_STATUS: i16 = 1;
const TEMPLATE_MANIFESTS: [&str; 4] = [
    include_str!("../../../../templates/llm-chat/template.json"),
    include_str!("../../../../templates/knowledge-base-chat/template.json"),
    include_str!("../../../../templates/agent-workspace/template.json"),
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
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerPackageResp {
    pub package_id: String,
    pub template: DeliveryTemplate,
    pub tenant_config: CustomerTenantConfig,
    pub branding: TemplateBranding,
    pub roles: Vec<TemplateRole>,
    pub menus: Vec<TemplateMenu>,
    pub prompts: Vec<TemplatePrompt>,
    pub skills: Vec<TemplateSkill>,
    pub connectors: Vec<TemplateConnector>,
    pub plugins: Vec<TemplatePlugin>,
    pub triggers: Vec<TemplateTrigger>,
    pub eval_sets: Vec<TemplateEvalSet>,
    pub deployment_checklist: Vec<String>,
    pub deployment_steps: Vec<String>,
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
    };
    let package_id = format!(
        "pkg_{}_{}",
        template.code,
        slug_component(&tenant_config.customer_name)
    );
    let deployment_steps = build_deployment_steps(&tenant_config, &template);

    Ok(CustomerPackageResp {
        package_id,
        template: template.clone(),
        tenant_config,
        branding,
        roles: template.roles.clone(),
        menus: template.menus.clone(),
        prompts: template.prompts.clone(),
        skills: template.skills.clone(),
        connectors: template.connectors.clone(),
        plugins: template.plugins.clone(),
        triggers: template.triggers.clone(),
        eval_sets: template.eval_sets.clone(),
        deployment_checklist: template.deployment_checklist.clone(),
        deployment_steps,
    })
}

fn validate_template_manifest(template: &DeliveryTemplate) -> Result<(), AppError> {
    if template.code.trim().is_empty()
        || template.name.trim().is_empty()
        || template.category.trim().is_empty()
        || template.frontend_entry.trim().is_empty()
    {
        return Err(AppError::bad_request("交付模板基础字段不能为空"));
    }
    if template.roles.is_empty()
        || template.menus.is_empty()
        || template.eval_sets.is_empty()
        || template.branding.brand_name.trim().is_empty()
        || template.deployment_checklist.is_empty()
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
    steps
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

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_template_size() -> u64 {
    DEFAULT_TEMPLATE_PAGE_SIZE
}

fn default_enabled_status() -> Option<i16> {
    Some(ENABLED_STATUS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_templates_include_all_m5_defaults() {
        let templates = delivery_templates().unwrap();
        let codes = templates
            .iter()
            .map(|template| template.code.as_str())
            .collect::<Vec<_>>();

        assert_eq!(templates.len(), 4);
        assert!(codes.contains(&"llm_chat"));
        assert!(codes.contains(&"knowledge_base_chat"));
        assert!(codes.contains(&"agent_workspace"));
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
    }
}
