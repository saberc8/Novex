use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool};

use crate::shared::error::AppError;

const STUDIO_ARTIFACT_LIMIT: i64 = 50;

#[derive(Debug, Clone)]
pub struct AiStudioRepository {
    db: PgPool,
}

#[derive(Debug, Clone, FromRow)]
pub struct StudioActionRow {
    pub id: i64,
    pub tenant_id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub surface: String,
    pub artifact_type: String,
    pub plugin_code: Option<String>,
    pub skill_code: Option<String>,
    pub permission_code: String,
    pub model_route_policy: Value,
    pub input_schema: Value,
    pub output_schema: Value,
    pub renderer: String,
    pub sort: i32,
    pub status: i16,
    pub metadata: Value,
    pub create_time: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub struct StudioArtifactRow {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: Option<i64>,
    pub session_id: Option<i64>,
    pub run_id: Option<i64>,
    pub rag_trace_id: Option<i64>,
    pub action_code: String,
    pub artifact_type: String,
    pub title: String,
    pub content_json: Value,
    pub content_text: String,
    pub source_snapshot: Value,
    pub citations: Value,
    pub version: i32,
    pub status: i16,
    pub metadata: Value,
    pub create_user: i64,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct StudioArtifactSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: Option<i64>,
    pub session_id: Option<i64>,
    pub run_id: Option<i64>,
    pub rag_trace_id: Option<i64>,
    pub action_code: String,
    pub artifact_type: String,
    pub title: String,
    pub content_json: Value,
    pub content_text: String,
    pub source_snapshot: Value,
    pub citations: Value,
    pub version: i32,
    pub status: i16,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

impl AiStudioRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn list_actions(
        &self,
        tenant_id: i64,
        surface: Option<&str>,
    ) -> Result<Vec<StudioActionRow>, AppError> {
        let rows = if let Some(surface) = surface {
            sqlx::query_as::<_, StudioActionRow>(
                r#"
SELECT
    id, tenant_id, code, name, description, surface, artifact_type, plugin_code,
    skill_code, permission_code, model_route_policy, input_schema, output_schema,
    renderer, sort, status, metadata, create_time
FROM ai_studio_action
WHERE tenant_id = $1
  AND surface = $2
  AND status = 1
ORDER BY sort ASC, id ASC;
"#,
            )
            .bind(tenant_id)
            .bind(surface)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query_as::<_, StudioActionRow>(
                r#"
SELECT
    id, tenant_id, code, name, description, surface, artifact_type, plugin_code,
    skill_code, permission_code, model_route_policy, input_schema, output_schema,
    renderer, sort, status, metadata, create_time
FROM ai_studio_action
WHERE tenant_id = $1
  AND status = 1
ORDER BY surface ASC, sort ASC, id ASC;
"#,
            )
            .bind(tenant_id)
            .fetch_all(&self.db)
            .await?
        };
        Ok(rows)
    }

    pub async fn find_action(
        &self,
        tenant_id: i64,
        code: &str,
    ) -> Result<Option<StudioActionRow>, AppError> {
        Ok(sqlx::query_as::<_, StudioActionRow>(
            r#"
SELECT
    id, tenant_id, code, name, description, surface, artifact_type, plugin_code,
    skill_code, permission_code, model_route_policy, input_schema, output_schema,
    renderer, sort, status, metadata, create_time
FROM ai_studio_action
WHERE tenant_id = $1
  AND code = $2
  AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(code)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn insert_artifact(&self, record: &StudioArtifactSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_studio_artifact (
    id, tenant_id, dataset_id, session_id, run_id, rag_trace_id, action_code,
    artifact_type, title, content_json, content_text, source_snapshot, citations,
    version, status, metadata, create_user, create_time, update_user, update_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7,
    $8, $9, $10, $11, $12, $13,
    $14, $15, $16, $17, $18, $17, $18
);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.dataset_id)
        .bind(record.session_id)
        .bind(record.run_id)
        .bind(record.rag_trace_id)
        .bind(&record.action_code)
        .bind(&record.artifact_type)
        .bind(&record.title)
        .bind(&record.content_json)
        .bind(&record.content_text)
        .bind(&record.source_snapshot)
        .bind(&record.citations)
        .bind(record.version)
        .bind(record.status)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn list_dataset_artifacts(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
    ) -> Result<Vec<StudioArtifactRow>, AppError> {
        Ok(sqlx::query_as::<_, StudioArtifactRow>(
            r#"
SELECT
    id, tenant_id, dataset_id, session_id, run_id, rag_trace_id, action_code,
    artifact_type, title, content_json, content_text, source_snapshot, citations,
    version, status, metadata, create_user, create_time, update_time
FROM ai_studio_artifact
WHERE tenant_id = $1
  AND create_user = $2
  AND dataset_id = $3
  AND status = 1
ORDER BY create_time DESC, id DESC
LIMIT $4;
"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(dataset_id)
        .bind(STUDIO_ARTIFACT_LIMIT)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn get_artifact(
        &self,
        tenant_id: i64,
        user_id: i64,
        artifact_id: i64,
    ) -> Result<Option<StudioArtifactRow>, AppError> {
        Ok(sqlx::query_as::<_, StudioArtifactRow>(
            r#"
SELECT
    id, tenant_id, dataset_id, session_id, run_id, rag_trace_id, action_code,
    artifact_type, title, content_json, content_text, source_snapshot, citations,
    version, status, metadata, create_user, create_time, update_time
FROM ai_studio_artifact
WHERE tenant_id = $1
  AND create_user = $2
  AND id = $3
  AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(artifact_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn soft_delete_artifact(
        &self,
        tenant_id: i64,
        user_id: i64,
        artifact_id: i64,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            r#"
UPDATE ai_studio_artifact
SET status = 0,
    update_user = $2,
    update_time = NOW()
WHERE tenant_id = $1
  AND create_user = $2
  AND id = $3
  AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(artifact_id)
        .execute(&self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
