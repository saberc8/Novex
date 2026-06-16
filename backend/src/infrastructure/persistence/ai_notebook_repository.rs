use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool};

use crate::shared::error::AppError;

const NOTEBOOK_WORKSPACE_LIMIT: i64 = 100;
const NOTEBOOK_SOURCE_LIMIT: i64 = 500;
const NOTEBOOK_ARTIFACT_LIMIT: i64 = 100;

#[derive(Debug, Clone)]
pub struct AiNotebookRepository {
    db: PgPool,
}

#[derive(Debug, Clone, FromRow)]
pub struct NotebookWorkspaceRow {
    pub id: i64,
    pub tenant_id: i64,
    pub owner_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub metadata: Value,
    pub status: i16,
    pub create_user: i64,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct NotebookSourceRow {
    pub id: i64,
    pub tenant_id: i64,
    pub workspace_id: i64,
    pub source_type: String,
    pub knowledge_dataset_id: Option<i64>,
    pub knowledge_document_id: Option<i64>,
    pub title: String,
    pub citation_metadata: Value,
    pub metadata: Value,
    pub status: i16,
    pub create_user: i64,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct NotebookArtifactRow {
    pub id: i64,
    pub tenant_id: i64,
    pub workspace_id: i64,
    pub artifact_kind: String,
    pub title: String,
    pub content_json: Value,
    pub content_text: String,
    pub citation_payload: Value,
    pub source_trace_id: Option<String>,
    pub metadata: Value,
    pub status: i16,
    pub create_user: i64,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct NotebookWorkspaceSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub owner_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub metadata: Value,
    pub status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NotebookSourceSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub workspace_id: i64,
    pub source_type: String,
    pub knowledge_dataset_id: Option<i64>,
    pub knowledge_document_id: Option<i64>,
    pub title: String,
    pub citation_metadata: Value,
    pub metadata: Value,
    pub status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NotebookArtifactSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub workspace_id: i64,
    pub artifact_kind: String,
    pub title: String,
    pub content_json: Value,
    pub content_text: String,
    pub citation_payload: Value,
    pub source_trace_id: Option<String>,
    pub metadata: Value,
    pub status: i16,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

impl AiNotebookRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn create_workspace(
        &self,
        record: &NotebookWorkspaceSaveRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_notebook_workspace (
    id, tenant_id, owner_id, name, description, metadata, status, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.owner_id)
        .bind(&record.name)
        .bind(&record.description)
        .bind(&record.metadata)
        .bind(record.status)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn list_workspaces(
        &self,
        tenant_id: i64,
        owner_id: i64,
    ) -> Result<Vec<NotebookWorkspaceRow>, AppError> {
        Ok(sqlx::query_as::<_, NotebookWorkspaceRow>(
            r#"
SELECT
    id, tenant_id, owner_id, name, description, metadata, status, create_user,
    create_time, update_time
FROM ai_notebook_workspace
WHERE tenant_id = $1
  AND owner_id = $2
  AND status = 1
ORDER BY create_time DESC, id DESC
LIMIT $3;
"#,
        )
        .bind(tenant_id)
        .bind(owner_id)
        .bind(NOTEBOOK_WORKSPACE_LIMIT)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn get_workspace(
        &self,
        tenant_id: i64,
        owner_id: i64,
        workspace_id: i64,
    ) -> Result<Option<NotebookWorkspaceRow>, AppError> {
        Ok(sqlx::query_as::<_, NotebookWorkspaceRow>(
            r#"
SELECT
    id, tenant_id, owner_id, name, description, metadata, status, create_user,
    create_time, update_time
FROM ai_notebook_workspace
WHERE tenant_id = $1
  AND owner_id = $2
  AND id = $3
  AND status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(owner_id)
        .bind(workspace_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn add_source(&self, record: &NotebookSourceSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_notebook_source (
    id, tenant_id, workspace_id, source_type, knowledge_dataset_id,
    knowledge_document_id, title, citation_metadata, metadata, status,
    create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.workspace_id)
        .bind(&record.source_type)
        .bind(record.knowledge_dataset_id)
        .bind(record.knowledge_document_id)
        .bind(&record.title)
        .bind(&record.citation_metadata)
        .bind(&record.metadata)
        .bind(record.status)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn list_sources(
        &self,
        tenant_id: i64,
        owner_id: i64,
        workspace_id: i64,
    ) -> Result<Vec<NotebookSourceRow>, AppError> {
        Ok(sqlx::query_as::<_, NotebookSourceRow>(
            r#"
SELECT
    s.id, s.tenant_id, s.workspace_id, s.source_type, s.knowledge_dataset_id,
    s.knowledge_document_id, s.title, s.citation_metadata, s.metadata, s.status,
    s.create_user, s.create_time, s.update_time
FROM ai_notebook_source s
JOIN ai_notebook_workspace w
  ON w.tenant_id = s.tenant_id
 AND w.id = s.workspace_id
WHERE s.tenant_id = $1
  AND s.workspace_id = $2
  AND s.status = 1
  AND w.owner_id = $3
  AND w.status = 1
ORDER BY s.create_time DESC, s.id DESC
LIMIT $4;
"#,
        )
        .bind(tenant_id)
        .bind(workspace_id)
        .bind(owner_id)
        .bind(NOTEBOOK_SOURCE_LIMIT)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn get_source(
        &self,
        tenant_id: i64,
        owner_id: i64,
        workspace_id: i64,
        source_id: i64,
    ) -> Result<Option<NotebookSourceRow>, AppError> {
        Ok(sqlx::query_as::<_, NotebookSourceRow>(
            r#"
SELECT
    s.id, s.tenant_id, s.workspace_id, s.source_type, s.knowledge_dataset_id,
    s.knowledge_document_id, s.title, s.citation_metadata, s.metadata, s.status,
    s.create_user, s.create_time, s.update_time
FROM ai_notebook_source s
JOIN ai_notebook_workspace w
  ON w.tenant_id = s.tenant_id
 AND w.id = s.workspace_id
WHERE s.tenant_id = $1
  AND s.workspace_id = $2
  AND s.id = $3
  AND s.status = 1
  AND w.owner_id = $4
  AND w.status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(workspace_id)
        .bind(source_id)
        .bind(owner_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn create_artifact(
        &self,
        record: &NotebookArtifactSaveRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_notebook_artifact (
    id, tenant_id, workspace_id, artifact_kind, title, content_json, content_text,
    citation_payload, source_trace_id, metadata, status, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.workspace_id)
        .bind(&record.artifact_kind)
        .bind(&record.title)
        .bind(&record.content_json)
        .bind(&record.content_text)
        .bind(&record.citation_payload)
        .bind(&record.source_trace_id)
        .bind(&record.metadata)
        .bind(record.status)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn list_artifacts(
        &self,
        tenant_id: i64,
        owner_id: i64,
        workspace_id: i64,
    ) -> Result<Vec<NotebookArtifactRow>, AppError> {
        Ok(sqlx::query_as::<_, NotebookArtifactRow>(
            r#"
SELECT
    a.id, a.tenant_id, a.workspace_id, a.artifact_kind, a.title, a.content_json,
    a.content_text, a.citation_payload, a.source_trace_id, a.metadata, a.status,
    a.create_user, a.create_time, a.update_time
FROM ai_notebook_artifact a
JOIN ai_notebook_workspace w
  ON w.tenant_id = a.tenant_id
 AND w.id = a.workspace_id
WHERE a.tenant_id = $1
  AND a.workspace_id = $2
  AND a.status = 1
  AND w.owner_id = $3
  AND w.status = 1
ORDER BY a.create_time DESC, a.id DESC
LIMIT $4;
"#,
        )
        .bind(tenant_id)
        .bind(workspace_id)
        .bind(owner_id)
        .bind(NOTEBOOK_ARTIFACT_LIMIT)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn get_artifact(
        &self,
        tenant_id: i64,
        owner_id: i64,
        workspace_id: i64,
        artifact_id: i64,
    ) -> Result<Option<NotebookArtifactRow>, AppError> {
        Ok(sqlx::query_as::<_, NotebookArtifactRow>(
            r#"
SELECT
    a.id, a.tenant_id, a.workspace_id, a.artifact_kind, a.title, a.content_json,
    a.content_text, a.citation_payload, a.source_trace_id, a.metadata, a.status,
    a.create_user, a.create_time, a.update_time
FROM ai_notebook_artifact a
JOIN ai_notebook_workspace w
  ON w.tenant_id = a.tenant_id
 AND w.id = a.workspace_id
WHERE a.tenant_id = $1
  AND a.workspace_id = $2
  AND a.id = $3
  AND a.status = 1
  AND w.owner_id = $4
  AND w.status = 1;
"#,
        )
        .bind(tenant_id)
        .bind(workspace_id)
        .bind(artifact_id)
        .bind(owner_id)
        .fetch_optional(&self.db)
        .await?)
    }
}
