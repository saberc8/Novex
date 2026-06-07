use chrono::NaiveDateTime;
use novex_plugin::{builtin_plugin_manifest, validate_plugin_manifest, PluginCapabilityKind};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Postgres, Transaction};

use crate::shared::{error::AppError, id::next_id};

#[derive(Debug, Clone)]
pub struct AiTemplateRepository {
    db: PgPool,
}

#[derive(Debug, Clone)]
pub struct CustomerPackageApplyRecord {
    pub package_id: String,
    pub tenant_code: String,
    pub tenant_name: String,
    pub plan_code: String,
    pub template_code: String,
    pub customer_name: String,
    pub app_name: String,
    pub package_payload: Value,
    pub provisioning_plan: Value,
    pub applied_steps: Value,
    pub pending_operator_steps: Value,
    pub frontend_config: CustomerPackageFrontendConfigApplyRecord,
    pub roles: Vec<CustomerPackageRoleApplyRecord>,
    pub menus: Vec<CustomerPackageMenuApplyRecord>,
    pub skills: Vec<CustomerPackageSkillApplyRecord>,
    pub connectors: Vec<CustomerPackageConnectorApplyRecord>,
    pub plugins: Vec<CustomerPackagePluginApplyRecord>,
    pub triggers: Vec<CustomerPackageTriggerApplyRecord>,
    pub eval_sets: Vec<CustomerPackageEvalSetApplyRecord>,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct CustomerPackageRoleApplyRecord {
    pub code: String,
    pub name: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CustomerPackageMenuApplyRecord {
    pub code: String,
    pub title: String,
    pub path: String,
    pub permission: String,
}

#[derive(Debug, Clone)]
pub struct CustomerPackageFrontendConfigApplyRecord {
    pub app: String,
    pub entry: String,
    pub entry_url: String,
    pub default_path: String,
    pub branding: Value,
    pub navigation: Value,
    pub allowed_roles: Value,
}

#[derive(Debug, Clone)]
pub struct CustomerPackageSkillApplyRecord {
    pub code: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct CustomerPackageConnectorApplyRecord {
    pub code: String,
    pub name: String,
    pub kind: String,
    pub auth_type: String,
}

#[derive(Debug, Clone)]
pub struct CustomerPackagePluginApplyRecord {
    pub code: String,
    pub name: String,
    pub runtime: String,
}

#[derive(Debug, Clone)]
pub struct CustomerPackageTriggerApplyRecord {
    pub code: String,
    pub name: String,
    pub source_type: String,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct CustomerPackageEvalSetApplyRecord {
    pub code: String,
    pub name: String,
    pub case_count: i32,
    pub metrics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedCustomerPackageRecord {
    pub tenant_id: i64,
    pub tenant_code: String,
}

#[derive(Debug, Clone)]
pub struct TemplateSmokeRunSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub template_code: String,
    pub package_id: Option<String>,
    pub smoke_script: String,
    pub status: String,
    pub dry_run: bool,
    pub total_checks: i32,
    pub passed_checks: i32,
    pub failed_checks: i32,
    pub result_payload: Value,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct TemplateSmokeResultSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub run_id: i64,
    pub check_code: String,
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub duration_ms: i64,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct TenantApplyRow {
    id: i64,
    code: String,
}

#[derive(Debug, Clone, FromRow)]
struct RoleApplyRow {
    id: i64,
}

#[derive(Debug, Clone, FromRow)]
struct MenuApplyRow {
    id: i64,
}

#[derive(Debug, Clone, FromRow)]
struct PluginApplyRow {
    id: i64,
}

#[derive(Debug, Clone, FromRow)]
struct PluginVersionApplyRow {
    id: i64,
}

#[derive(Debug, Clone, FromRow)]
struct EvalDatasetApplyRow {
    id: i64,
}

#[derive(Debug, Clone, FromRow)]
struct SeedEvalCaseApplyRow {
    case_code: String,
    target_kind: String,
    metric_kind: String,
    prompt: String,
    expected_payload: Value,
    tags: Value,
    sort: i32,
}

impl AiTemplateRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn apply_customer_package(
        &self,
        record: &CustomerPackageApplyRecord,
    ) -> Result<AppliedCustomerPackageRecord, AppError> {
        let mut tx = self.db.begin().await?;
        let tenant = sqlx::query_as::<_, TenantApplyRow>(
            r#"
INSERT INTO sys_tenant (
    id, code, name, status, plan_code, metadata, create_user, create_time
)
VALUES ($1, $2, $3, 1, $4, $5, $6, $7)
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    plan_code = EXCLUDED.plan_code,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time
RETURNING id, code;
"#,
        )
        .bind(next_id())
        .bind(&record.tenant_code)
        .bind(&record.tenant_name)
        .bind(&record.plan_code)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .fetch_one(&mut *tx)
        .await?;

        let parent_menu_id = upsert_template_menu_parent(&mut tx, record, tenant.id).await?;
        for (index, menu) in record.menus.iter().enumerate() {
            upsert_template_menu(&mut tx, record, parent_menu_id, menu, index as i32).await?;
        }
        for (index, role) in record.roles.iter().enumerate() {
            let role_id = upsert_template_role(&mut tx, record, tenant.id, role, index as i32)
                .await?
                .id;
            bind_tenant_role(&mut tx, record, tenant.id, role_id).await?;
            for permission in normalized_permissions(&role.permissions) {
                bind_role_permission(&mut tx, record, role_id, &permission).await?;
            }
        }
        upsert_customer_frontend_config(&mut tx, record, tenant.id).await?;
        for skill in &record.skills {
            upsert_template_skill(&mut tx, record, tenant.id, skill).await?;
        }
        for connector in &record.connectors {
            upsert_template_connector(&mut tx, record, tenant.id, connector).await?;
        }
        for plugin in &record.plugins {
            upsert_template_plugin(&mut tx, record, tenant.id, plugin).await?;
        }
        for trigger in &record.triggers {
            upsert_template_trigger(&mut tx, record, tenant.id, trigger).await?;
        }
        for eval_set in &record.eval_sets {
            upsert_template_eval_set(&mut tx, record, tenant.id, eval_set).await?;
        }

        sqlx::query(
            r#"
INSERT INTO ai_customer_package (
    id, package_id, tenant_id, template_code, customer_name, app_name,
    status, package_payload, provisioning_plan, applied_steps,
    pending_operator_steps, metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6,
    'applied', $7, $8, $9,
    $10, $11, $12, $13
)
ON CONFLICT (package_id) DO UPDATE
SET tenant_id = EXCLUDED.tenant_id,
    template_code = EXCLUDED.template_code,
    customer_name = EXCLUDED.customer_name,
    app_name = EXCLUDED.app_name,
    status = EXCLUDED.status,
    package_payload = EXCLUDED.package_payload,
    provisioning_plan = EXCLUDED.provisioning_plan,
    applied_steps = EXCLUDED.applied_steps,
    pending_operator_steps = EXCLUDED.pending_operator_steps,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
        )
        .bind(next_id())
        .bind(&record.package_id)
        .bind(tenant.id)
        .bind(&record.template_code)
        .bind(&record.customer_name)
        .bind(&record.app_name)
        .bind(&record.package_payload)
        .bind(&record.provisioning_plan)
        .bind(&record.applied_steps)
        .bind(&record.pending_operator_steps)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(AppliedCustomerPackageRecord {
            tenant_id: tenant.id,
            tenant_code: tenant.code,
        })
    }

    pub async fn create_template_smoke_run(
        &self,
        record: &TemplateSmokeRunSaveRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_template_smoke_run (
    id, tenant_id, template_code, package_id, smoke_script, status,
    dry_run, total_checks, passed_checks, failed_checks, result_payload,
    metadata, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.template_code)
        .bind(&record.package_id)
        .bind(&record.smoke_script)
        .bind(&record.status)
        .bind(record.dry_run)
        .bind(record.total_checks)
        .bind(record.passed_checks)
        .bind(record.failed_checks)
        .bind(&record.result_payload)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn create_template_smoke_result(
        &self,
        record: &TemplateSmokeResultSaveRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_template_smoke_result (
    id, tenant_id, run_id, check_code, name, workdir, command,
    status, exit_code, stdout, stderr, duration_ms, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.run_id)
        .bind(&record.check_code)
        .bind(&record.name)
        .bind(&record.workdir)
        .bind(&record.command)
        .bind(&record.status)
        .bind(record.exit_code)
        .bind(&record.stdout)
        .bind(&record.stderr)
        .bind(record.duration_ms)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;

        Ok(())
    }
}

async fn upsert_template_menu_parent(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
) -> Result<i64, AppError> {
    let title = bounded_label(
        &format!("tpl:{}:{}", record.tenant_code, record.template_code),
        30,
    );
    let path = format!("/templates/{}/{}", record.tenant_code, record.template_code);
    let name = bounded_label(&format!("TenantTemplate{tenant_id}"), 50);
    let row = sqlx::query_as::<_, MenuApplyRow>(
        r#"
INSERT INTO sys_menu (
    id, title, parent_id, type, path, name, component, redirect, icon,
    is_external, is_cache, is_hidden, permission, sort, status,
    create_user, create_time
)
VALUES (
    $1, $2, 3000, 2, $3, $4, NULL, NULL, 'apps',
    FALSE, FALSE, TRUE, NULL, 90, 1,
    $5, $6
)
ON CONFLICT (title, parent_id) DO UPDATE
SET path = EXCLUDED.path,
    name = EXCLUDED.name,
    is_hidden = EXCLUDED.is_hidden,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time
RETURNING id;
"#,
    )
    .bind(next_id())
    .bind(title)
    .bind(path)
    .bind(name)
    .bind(record.user_id)
    .bind(record.now)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row.id)
}

async fn upsert_template_menu(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    parent_id: i64,
    menu: &CustomerPackageMenuApplyRecord,
    index: i32,
) -> Result<i64, AppError> {
    let title = bounded_label(&format!("{}:{}", menu.code, menu.title), 30);
    let row = sqlx::query_as::<_, MenuApplyRow>(
        r#"
INSERT INTO sys_menu (
    id, title, parent_id, type, path, name, component, redirect, icon,
    is_external, is_cache, is_hidden, permission, sort, status,
    create_user, create_time
)
VALUES (
    $1, $2, $3, 3, $4, NULL, NULL, NULL, NULL,
    FALSE, FALSE, TRUE, $5, $6, 1,
    $7, $8
)
ON CONFLICT (title, parent_id) DO UPDATE
SET path = EXCLUDED.path,
    permission = EXCLUDED.permission,
    sort = EXCLUDED.sort,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time
RETURNING id;
"#,
    )
    .bind(next_id())
    .bind(title)
    .bind(parent_id)
    .bind(&menu.path)
    .bind(&menu.permission)
    .bind(index + 1)
    .bind(record.user_id)
    .bind(record.now)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row.id)
}

async fn upsert_template_role(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    role: &CustomerPackageRoleApplyRecord,
    index: i32,
) -> Result<RoleApplyRow, AppError> {
    let code = tenant_role_code(tenant_id, &role.code);
    let name = tenant_role_name(tenant_id, &role.name);
    sqlx::query_as::<_, RoleApplyRow>(
        r#"
INSERT INTO sys_role (
    id, name, code, data_scope, description, sort,
    is_system, menu_check_strictly, dept_check_strictly,
    create_user, create_time, status
)
VALUES (
    $1, $2, $3, 4, $4, $5,
    FALSE, TRUE, TRUE,
    $6, $7, 1
)
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    sort = EXCLUDED.sort,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time
RETURNING id;
"#,
    )
    .bind(next_id())
    .bind(name)
    .bind(code)
    .bind(format!(
        "Template role {} for package {}",
        role.code, record.package_id
    ))
    .bind(100 + index)
    .bind(record.user_id)
    .bind(record.now)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn bind_tenant_role(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    role_id: i64,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO sys_tenant_role (
    id, tenant_id, role_id, status, create_user, create_time
)
VALUES ($1, $2, $3, 1, $4, $5)
ON CONFLICT (tenant_id, role_id) DO UPDATE
SET status = 1,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(role_id)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn bind_role_permission(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    role_id: i64,
    permission: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO sys_role_menu (role_id, menu_id)
SELECT $2, id
FROM sys_menu
WHERE permission = $1
  AND status = 1
ON CONFLICT DO NOTHING;
"#,
    )
    .bind(permission)
    .bind(role_id)
    .execute(&mut **tx)
    .await?;

    let _ = record;
    Ok(())
}

async fn upsert_customer_frontend_config(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_customer_frontend_config (
    id, package_id, tenant_id, template_code, app_id, frontend_entry,
    entry_url, default_path, branding, navigation, allowed_roles,
    status, metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6,
    $7, $8, $9, $10, $11,
    1,
    jsonb_build_object(
        'source', 'template_apply',
        'customerName', $12::TEXT,
        'appName', $13::TEXT
    ),
    $14, $15
)
ON CONFLICT (package_id) DO UPDATE
SET tenant_id = EXCLUDED.tenant_id,
    template_code = EXCLUDED.template_code,
    app_id = EXCLUDED.app_id,
    frontend_entry = EXCLUDED.frontend_entry,
    entry_url = EXCLUDED.entry_url,
    default_path = EXCLUDED.default_path,
    branding = EXCLUDED.branding,
    navigation = EXCLUDED.navigation,
    allowed_roles = EXCLUDED.allowed_roles,
    status = EXCLUDED.status,
    metadata = ai_customer_frontend_config.metadata || EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(&record.package_id)
    .bind(tenant_id)
    .bind(&record.template_code)
    .bind(&record.frontend_config.app)
    .bind(&record.frontend_config.entry)
    .bind(&record.frontend_config.entry_url)
    .bind(&record.frontend_config.default_path)
    .bind(&record.frontend_config.branding)
    .bind(&record.frontend_config.navigation)
    .bind(&record.frontend_config.allowed_roles)
    .bind(&record.customer_name)
    .bind(&record.app_name)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_template_skill(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    skill: &CustomerPackageSkillApplyRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
WITH seed AS (
    SELECT model_route_policy, capability_refs, metadata
    FROM ai_skill
    WHERE tenant_id = 1 AND code = $3
    LIMIT 1
),
payload AS (
    SELECT
        COALESCE((SELECT model_route_policy FROM seed), '{}'::jsonb) AS model_route_policy,
        COALESCE((SELECT capability_refs FROM seed), '[]'::jsonb) AS capability_refs,
        COALESCE((SELECT metadata FROM seed), '{}'::jsonb)
            || jsonb_build_object(
                'source', 'template_apply',
                'templateCode', $6::TEXT,
                'packageId', $7::TEXT
            ) AS metadata
)
INSERT INTO ai_skill (
    id, tenant_id, code, name, description, status, model_route_policy,
    capability_refs, metadata, create_user, create_time
)
SELECT
    $1, $2, $3, $4, $5, 1, p.model_route_policy,
    p.capability_refs, p.metadata, $8, $9
FROM payload AS p
ON CONFLICT (tenant_id, code) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    status = EXCLUDED.status,
    model_route_policy = EXCLUDED.model_route_policy,
    capability_refs = EXCLUDED.capability_refs,
    metadata = ai_skill.metadata || EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(&skill.code)
    .bind(&skill.name)
    .bind(&skill.description)
    .bind(&record.template_code)
    .bind(&record.package_id)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_template_connector(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    connector: &CustomerPackageConnectorApplyRecord,
) -> Result<(), AppError> {
    let auth_type = connector_auth_type(&connector.auth_type);
    sqlx::query(
        r#"
WITH seed AS (
    SELECT connector_kind, credential_scope, auth_type, description, metadata
    FROM ai_connector
    WHERE tenant_id = 1 AND code = $3
    LIMIT 1
),
payload AS (
    SELECT
        COALESCE((SELECT connector_kind FROM seed), $5::TEXT) AS connector_kind,
        COALESCE((SELECT credential_scope FROM seed), 'tenant') AS credential_scope,
        COALESCE((SELECT auth_type FROM seed), $6::TEXT) AS auth_type,
        COALESCE((SELECT description FROM seed), 'Template connector enabled for customer delivery.') AS description,
        COALESCE((SELECT metadata FROM seed), '{}'::jsonb)
            || jsonb_build_object(
                'source', 'template_apply',
                'templateCode', $7::TEXT,
                'packageId', $8::TEXT,
                'credentialPolicy', 'bind_via_secret_control_plane'
            ) AS metadata
)
INSERT INTO ai_connector (
    id, tenant_id, code, name, description, connector_kind, credential_scope,
    auth_type, status, metadata, create_user, create_time
)
SELECT
    $1, $2, $3, $4, p.description, p.connector_kind, p.credential_scope,
    p.auth_type, 1, p.metadata, $9, $10
FROM payload AS p
ON CONFLICT (tenant_id, code) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    connector_kind = EXCLUDED.connector_kind,
    credential_scope = EXCLUDED.credential_scope,
    auth_type = EXCLUDED.auth_type,
    status = EXCLUDED.status,
    metadata = ai_connector.metadata || EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(&connector.code)
    .bind(&connector.name)
    .bind(&connector.kind)
    .bind(&auth_type)
    .bind(&record.template_code)
    .bind(&record.package_id)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_template_plugin(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    plugin: &CustomerPackagePluginApplyRecord,
) -> Result<(), AppError> {
    let Some(manifest) = builtin_plugin_manifest(&plugin.code) else {
        return Err(AppError::bad_request(format!(
            "交付模板插件未注册内置 manifest: {}",
            plugin.code
        )));
    };
    validate_plugin_manifest(&manifest)
        .map_err(|err| AppError::bad_request(format!("交付模板插件 manifest 无效: {err:?}")))?;

    let manifest_value = serde_json::to_value(&manifest).unwrap_or_else(|_| json!({}));
    let permission_grants =
        serde_json::to_value(&manifest.permission_grants).unwrap_or_else(|_| json!([]));
    let config = json!({
        "source": "template_apply",
        "templateCode": record.template_code,
        "packageId": record.package_id,
        "runtime": plugin.runtime,
        "credentialPolicy": "bind_via_secret_control_plane"
    });
    let plugin_row = sqlx::query_as::<_, PluginApplyRow>(
        r#"
INSERT INTO ai_plugin (
    id, tenant_id, code, name, version, runtime, status,
    manifest, metadata, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, 1,
    $7, jsonb_build_object(
        'source', 'template_apply',
        'templateCode', $8::TEXT,
        'packageId', $9::TEXT
    ),
    $10, $11
)
ON CONFLICT (tenant_id, code) DO UPDATE
SET name = EXCLUDED.name,
    version = EXCLUDED.version,
    runtime = EXCLUDED.runtime,
    status = EXCLUDED.status,
    manifest = EXCLUDED.manifest,
    metadata = ai_plugin.metadata || EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time
RETURNING id;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(&manifest.code)
    .bind(&plugin.name)
    .bind(&manifest.version)
    .bind(manifest.runtime.as_str())
    .bind(&manifest_value)
    .bind(&record.template_code)
    .bind(&record.package_id)
    .bind(record.user_id)
    .bind(record.now)
    .fetch_one(&mut **tx)
    .await?;

    let version_row = sqlx::query_as::<_, PluginVersionApplyRow>(
        r#"
INSERT INTO ai_plugin_version (
    id, tenant_id, plugin_id, version, runtime, manifest,
    status, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $8)
ON CONFLICT (tenant_id, plugin_id, version) DO UPDATE
SET runtime = EXCLUDED.runtime,
    manifest = EXCLUDED.manifest,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time
RETURNING id;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(plugin_row.id)
    .bind(&manifest.version)
    .bind(manifest.runtime.as_str())
    .bind(&manifest_value)
    .bind(record.user_id)
    .bind(record.now)
    .fetch_one(&mut **tx)
    .await?;

    for capability in &manifest.capabilities {
        if matches!(capability.kind, PluginCapabilityKind::Tool) {
            upsert_template_tool_from_seed(
                tx,
                record,
                tenant_id,
                &capability.code,
                &capability.permission_code,
            )
            .await?;
        }
        upsert_template_plugin_capability(
            tx,
            record,
            tenant_id,
            plugin_row.id,
            version_row.id,
            plugin,
            capability_kind(capability.kind),
            &capability.code,
            &capability.permission_code,
        )
        .await?;
    }

    sqlx::query(
        r#"
INSERT INTO ai_plugin_installation (
    id, tenant_id, plugin_id, plugin_version_id, enabled, install_source,
    permission_grants, config, installed_by, installed_at, status,
    create_user, create_time
)
VALUES ($1, $2, $3, $4, TRUE, 'template_apply', $5, $6, $7, $8, 1, $7, $8)
ON CONFLICT (tenant_id, plugin_id) DO UPDATE
SET plugin_version_id = EXCLUDED.plugin_version_id,
    enabled = EXCLUDED.enabled,
    install_source = EXCLUDED.install_source,
    permission_grants = EXCLUDED.permission_grants,
    config = ai_plugin_installation.config || EXCLUDED.config,
    installed_by = EXCLUDED.installed_by,
    installed_at = EXCLUDED.installed_at,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(plugin_row.id)
    .bind(version_row.id)
    .bind(&permission_grants)
    .bind(&config)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_template_tool_from_seed(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    tool_code: &str,
    permission_code: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
WITH seed AS (
    SELECT
        code, name, description, tool_kind, risk_level, approval_policy,
        permission_code, executor_kind, input_schema, output_schema, metadata
    FROM ai_tool
    WHERE tenant_id = 1 AND code = $3
    LIMIT 1
),
payload AS (
    SELECT
        COALESCE((SELECT name FROM seed), $3::TEXT) AS name,
        COALESCE((SELECT description FROM seed), 'Template plugin tool enabled for customer delivery.') AS description,
        COALESCE((SELECT tool_kind FROM seed), $4::TEXT) AS tool_kind,
        COALESCE((SELECT risk_level FROM seed), 1) AS risk_level,
        COALESCE((SELECT approval_policy FROM seed), 1) AS approval_policy,
        COALESCE((SELECT permission_code FROM seed), $5::TEXT) AS permission_code,
        COALESCE((SELECT executor_kind FROM seed), 'dry_run') AS executor_kind,
        COALESCE((SELECT input_schema FROM seed), '{}'::jsonb) AS input_schema,
        COALESCE((SELECT output_schema FROM seed), '{}'::jsonb) AS output_schema,
        COALESCE((SELECT metadata FROM seed), '{}'::jsonb)
            || jsonb_build_object(
                'source', 'template_apply',
                'templateCode', $6::TEXT,
                'packageId', $7::TEXT
            ) AS metadata
)
INSERT INTO ai_tool (
    id, tenant_id, code, name, description, tool_kind, risk_level,
    approval_policy, permission_code, executor_kind, input_schema,
    output_schema, status, metadata, create_user, create_time
)
SELECT
    $1, $2, $3, p.name, p.description, p.tool_kind, p.risk_level,
    p.approval_policy, p.permission_code, p.executor_kind, p.input_schema,
    p.output_schema, 1, p.metadata, $8, $9
FROM payload AS p
ON CONFLICT (tenant_id, code) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    tool_kind = EXCLUDED.tool_kind,
    risk_level = EXCLUDED.risk_level,
    approval_policy = EXCLUDED.approval_policy,
    permission_code = EXCLUDED.permission_code,
    executor_kind = EXCLUDED.executor_kind,
    input_schema = EXCLUDED.input_schema,
    output_schema = EXCLUDED.output_schema,
    status = EXCLUDED.status,
    metadata = ai_tool.metadata || EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(tool_code)
    .bind(tool_kind_for_code(tool_code))
    .bind(permission_code)
    .bind(&record.template_code)
    .bind(&record.package_id)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_template_plugin_capability(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    plugin_id: i64,
    plugin_version_id: i64,
    plugin: &CustomerPackagePluginApplyRecord,
    capability_kind: &str,
    capability_code: &str,
    permission_code: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_plugin_capability (
    id, tenant_id, plugin_id, plugin_version_id, capability_kind,
    capability_code, permission_code, metadata, status, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5,
    $6, $7,
    jsonb_build_object(
        'source', 'template_apply',
        'templateCode', $8::TEXT,
        'packageId', $9::TEXT,
        'pluginCode', $10::TEXT
    ),
    1, $11, $12
)
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code)
DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = ai_plugin_capability.metadata || EXCLUDED.metadata,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(plugin_id)
    .bind(plugin_version_id)
    .bind(capability_kind)
    .bind(capability_code)
    .bind(permission_code)
    .bind(&record.template_code)
    .bind(&record.package_id)
    .bind(&plugin.code)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_template_trigger(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    trigger: &CustomerPackageTriggerApplyRecord,
) -> Result<(), AppError> {
    let trigger_kind = trigger.source_type.trim().to_ascii_lowercase();
    let signature_required = trigger_kind == "webhook";
    let route_config = default_trigger_route_config(trigger);
    sqlx::query(
        r#"
WITH seed AS (
    SELECT
        description, signature_required, idempotency_required,
        route_config, metadata
    FROM ai_trigger
    WHERE tenant_id = 1 AND code = $3
    LIMIT 1
),
payload AS (
    SELECT
        COALESCE((SELECT description FROM seed), 'Template trigger enabled for customer delivery.') AS description,
        COALESCE((SELECT signature_required FROM seed), $7::BOOLEAN) AS signature_required,
        COALESCE((SELECT idempotency_required FROM seed), TRUE) AS idempotency_required,
        COALESCE((SELECT route_config FROM seed), $8::jsonb) AS route_config,
        COALESCE((SELECT metadata FROM seed), '{}'::jsonb)
            || jsonb_build_object(
                'source', 'template_apply',
                'templateCode', $9::TEXT,
                'packageId', $10::TEXT
            ) AS metadata
)
INSERT INTO ai_trigger (
    id, tenant_id, code, name, description, trigger_kind, target_kind,
    signature_required, idempotency_required, route_config, status,
    metadata, create_user, create_time
)
SELECT
    $1, $2, $3, $4, p.description, $5, $6,
    p.signature_required, p.idempotency_required, p.route_config, 1,
    p.metadata, $11, $12
FROM payload AS p
ON CONFLICT (tenant_id, code) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    trigger_kind = EXCLUDED.trigger_kind,
    target_kind = EXCLUDED.target_kind,
    signature_required = EXCLUDED.signature_required,
    idempotency_required = EXCLUDED.idempotency_required,
    route_config = EXCLUDED.route_config,
    status = EXCLUDED.status,
    metadata = ai_trigger.metadata || EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(&trigger.code)
    .bind(&trigger.name)
    .bind(&trigger_kind)
    .bind(&trigger.target)
    .bind(signature_required)
    .bind(&route_config)
    .bind(&record.template_code)
    .bind(&record.package_id)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_template_eval_set(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    eval_set: &CustomerPackageEvalSetApplyRecord,
) -> Result<(), AppError> {
    let metrics = json!(eval_set.metrics);
    let dataset = sqlx::query_as::<_, EvalDatasetApplyRow>(
        r#"
WITH source_dataset AS (
    SELECT description, target_scope, metadata
    FROM ai_eval_dataset
    WHERE tenant_id = 1 AND code = $3
    LIMIT 1
),
payload AS (
    SELECT
        COALESCE((SELECT description FROM source_dataset), 'Template eval set selected for customer delivery.') AS description,
        COALESCE((SELECT target_scope FROM source_dataset), $5::TEXT) AS target_scope,
        COALESCE((SELECT metadata FROM source_dataset), '{}'::jsonb)
            || jsonb_build_object(
                'source', 'template_apply',
                'templateCode', $6::TEXT,
                'packageId', $7::TEXT,
                'caseCount', $8::INTEGER,
                'metrics', $9::jsonb
            ) AS metadata
)
INSERT INTO ai_eval_dataset (
    id, tenant_id, code, name, description, target_scope,
    status, metadata, create_user, create_time
)
SELECT
    $1, $2, $3, $4, p.description, p.target_scope,
    1, p.metadata, $10, $11
FROM payload AS p
ON CONFLICT (tenant_id, code) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    target_scope = EXCLUDED.target_scope,
    status = EXCLUDED.status,
    metadata = ai_eval_dataset.metadata || EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time
RETURNING id;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(&eval_set.code)
    .bind(&eval_set.name)
    .bind(eval_target_scope(&eval_set.code))
    .bind(&record.template_code)
    .bind(&record.package_id)
    .bind(eval_set.case_count)
    .bind(&metrics)
    .bind(record.user_id)
    .bind(record.now)
    .fetch_one(&mut **tx)
    .await?;

    let cases = sqlx::query_as::<_, SeedEvalCaseApplyRow>(
        r#"
SELECT
    c.case_code,
    c.target_kind,
    c.metric_kind,
    c.prompt,
    c.expected_payload,
    c.tags,
    c.sort
FROM ai_eval_case AS c
JOIN ai_eval_dataset AS d ON d.id = c.dataset_id AND d.tenant_id = c.tenant_id
WHERE d.tenant_id = 1
  AND d.code = $1
  AND c.status = 1
ORDER BY c.sort ASC, c.id ASC;
"#,
    )
    .bind(&eval_set.code)
    .fetch_all(&mut **tx)
    .await?;

    for case in cases {
        upsert_template_eval_case(tx, record, tenant_id, dataset.id, &case).await?;
    }

    Ok(())
}

async fn upsert_template_eval_case(
    tx: &mut Transaction<'_, Postgres>,
    record: &CustomerPackageApplyRecord,
    tenant_id: i64,
    dataset_id: i64,
    case: &SeedEvalCaseApplyRow,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_eval_case (
    id, tenant_id, dataset_id, case_code, target_kind, metric_kind,
    prompt, expected_payload, tags, status, sort, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1, $10, $11, $12)
ON CONFLICT (dataset_id, case_code) DO UPDATE
SET target_kind = EXCLUDED.target_kind,
    metric_kind = EXCLUDED.metric_kind,
    prompt = EXCLUDED.prompt,
    expected_payload = EXCLUDED.expected_payload,
    tags = EXCLUDED.tags,
    status = EXCLUDED.status,
    sort = EXCLUDED.sort,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(next_id())
    .bind(tenant_id)
    .bind(dataset_id)
    .bind(&case.case_code)
    .bind(&case.target_kind)
    .bind(&case.metric_kind)
    .bind(&case.prompt)
    .bind(&case.expected_payload)
    .bind(&case.tags)
    .bind(case.sort)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn normalized_permissions(permissions: &[String]) -> Vec<String> {
    let mut values = permissions
        .iter()
        .map(|permission| permission.trim())
        .filter(|permission| !permission.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn tenant_role_code(tenant_id: i64, role_code: &str) -> String {
    bounded_label(&format!("t{tenant_id}_{}", compact_code(role_code)), 30)
}

fn tenant_role_name(tenant_id: i64, role_name: &str) -> String {
    bounded_label(&format!("T{tenant_id} {role_name}"), 30)
}

fn compact_code(value: &str) -> String {
    let code = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if code.is_empty() {
        "role".to_owned()
    } else {
        code
    }
}

fn bounded_label(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn connector_auth_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "oauth" => "oauth_app".to_owned(),
        "token" => "app_secret".to_owned(),
        "" => "none".to_owned(),
        other => other.to_owned(),
    }
}

fn capability_kind(kind: PluginCapabilityKind) -> &'static str {
    match kind {
        PluginCapabilityKind::Skill => "skill",
        PluginCapabilityKind::Tool => "tool",
        PluginCapabilityKind::Connector => "connector",
        PluginCapabilityKind::Trigger => "trigger",
        PluginCapabilityKind::OAuthClient => "oauth_client",
        PluginCapabilityKind::UiConfig => "ui_config",
        PluginCapabilityKind::EvalCase => "eval_case",
    }
}

fn tool_kind_for_code(code: &str) -> &'static str {
    if code.starts_with("media.") {
        "media"
    } else if code.contains(".message.") || code.starts_with("github.") {
        "connector"
    } else {
        "function"
    }
}

fn default_trigger_route_config(trigger: &CustomerPackageTriggerApplyRecord) -> Value {
    let kind = trigger.source_type.trim().to_ascii_lowercase();
    if kind == "webhook" {
        json!({
            "path": format!("/ai/triggers/webhook/{}", webhook_trigger_key(&trigger.code)),
            "signatureHeader": "X-Novex-Signature",
            "idempotencyHeader": "Idempotency-Key"
        })
    } else {
        json!({
            "jobKey": trigger.code
        })
    }
}

fn webhook_trigger_key(code: &str) -> String {
    code.trim()
        .strip_suffix(".webhook")
        .or_else(|| code.trim().strip_suffix("_webhook"))
        .unwrap_or_else(|| code.trim())
        .rsplit(['.', '_', '-'])
        .find(|part| !part.is_empty())
        .unwrap_or("webhook")
        .to_owned()
}

fn eval_target_scope(code: &str) -> &'static str {
    if code.starts_with("llm_") {
        "chat"
    } else if code.starts_with("knowledge_") {
        "knowledge"
    } else if code.starts_with("agent_") {
        "agent"
    } else if code.starts_with("training_") {
        "training"
    } else {
        "template"
    }
}
