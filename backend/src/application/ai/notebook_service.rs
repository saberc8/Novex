use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::system::{
        ensure_max_chars, format_datetime, format_optional_datetime, trim_to_none,
    },
    infrastructure::persistence::ai_notebook_repository::{
        AiNotebookRepository, NotebookArtifactRow, NotebookArtifactSaveRecord, NotebookSourceRow,
        NotebookSourceSaveRecord, NotebookWorkspaceRow, NotebookWorkspaceSaveRecord,
    },
    shared::{error::AppError, id::next_id},
};

const NOTEBOOK_STATUS_ACTIVE: i16 = 1;
const NOTEBOOK_WORKSPACE_NAME_MAX_CHARS: usize = 128;
const NOTEBOOK_DESCRIPTION_MAX_CHARS: usize = 2_000;
const NOTEBOOK_TITLE_MAX_CHARS: usize = 255;
const NOTEBOOK_KIND_MAX_CHARS: usize = 64;
const NOTEBOOK_SOURCE_TYPE_DATASET: &str = "dataset";
const NOTEBOOK_SOURCE_TYPE_DOCUMENT: &str = "document";

#[derive(Debug, Clone)]
pub struct NotebookService {
    repo: AiNotebookRepository,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookWorkspaceCommand {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookSourceCommand {
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub knowledge_dataset_id: Option<i64>,
    #[serde(default)]
    pub knowledge_document_id: Option<i64>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub citation_metadata: Value,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookArtifactCommand {
    #[serde(default)]
    pub artifact_kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content_json: Value,
    #[serde(default)]
    pub content_text: String,
    #[serde(default)]
    pub citation_payload: Value,
    #[serde(default)]
    pub source_trace_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookWorkspaceResp {
    pub id: i64,
    pub tenant_id: i64,
    pub owner_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub metadata: Value,
    pub status: i16,
    pub create_user: i64,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookSourceResp {
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
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookArtifactResp {
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
    pub create_time: String,
    pub update_time: String,
}

impl NotebookService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiNotebookRepository::new(db),
        }
    }

    pub async fn create_workspace(
        &self,
        tenant_id: i64,
        user_id: i64,
        command: NotebookWorkspaceCommand,
    ) -> Result<NotebookWorkspaceResp, AppError> {
        let command = normalize_workspace_command(command)?;
        let id = next_id();
        let now = Utc::now().naive_utc();
        let record = NotebookWorkspaceSaveRecord {
            id,
            tenant_id,
            owner_id: user_id,
            name: command.name,
            description: command.description,
            metadata: command.metadata,
            status: NOTEBOOK_STATUS_ACTIVE,
            user_id,
            now,
        };
        self.repo.create_workspace(&record).await?;
        let row = self
            .repo
            .get_workspace(tenant_id, user_id, id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(NotebookWorkspaceResp::from(row))
    }

    pub async fn list_workspaces(
        &self,
        tenant_id: i64,
        user_id: i64,
    ) -> Result<Vec<NotebookWorkspaceResp>, AppError> {
        let rows = self.repo.list_workspaces(tenant_id, user_id).await?;
        Ok(rows.into_iter().map(NotebookWorkspaceResp::from).collect())
    }

    pub async fn get_workspace(
        &self,
        tenant_id: i64,
        user_id: i64,
        workspace_id: i64,
    ) -> Result<NotebookWorkspaceResp, AppError> {
        ensure_positive_id("Notebook Workspace ID", workspace_id)?;
        let row = self
            .repo
            .get_workspace(tenant_id, user_id, workspace_id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(NotebookWorkspaceResp::from(row))
    }

    pub async fn add_source(
        &self,
        tenant_id: i64,
        user_id: i64,
        workspace_id: i64,
        command: NotebookSourceCommand,
    ) -> Result<NotebookSourceResp, AppError> {
        ensure_positive_id("Notebook Workspace ID", workspace_id)?;
        self.ensure_workspace(tenant_id, user_id, workspace_id)
            .await?;
        let command = normalize_source_command(command)?;
        let id = next_id();
        let now = Utc::now().naive_utc();
        let record = NotebookSourceSaveRecord {
            id,
            tenant_id,
            workspace_id,
            source_type: command.source_type,
            knowledge_dataset_id: command.knowledge_dataset_id,
            knowledge_document_id: command.knowledge_document_id,
            title: command.title,
            citation_metadata: command.citation_metadata,
            metadata: command.metadata,
            status: NOTEBOOK_STATUS_ACTIVE,
            user_id,
            now,
        };
        self.repo.add_source(&record).await?;
        let row = self
            .repo
            .get_source(tenant_id, user_id, workspace_id, id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(NotebookSourceResp::from(row))
    }

    pub async fn list_sources(
        &self,
        tenant_id: i64,
        user_id: i64,
        workspace_id: i64,
    ) -> Result<Vec<NotebookSourceResp>, AppError> {
        ensure_positive_id("Notebook Workspace ID", workspace_id)?;
        self.ensure_workspace(tenant_id, user_id, workspace_id)
            .await?;
        let rows = self
            .repo
            .list_sources(tenant_id, user_id, workspace_id)
            .await?;
        Ok(rows.into_iter().map(NotebookSourceResp::from).collect())
    }

    pub async fn create_artifact(
        &self,
        tenant_id: i64,
        user_id: i64,
        workspace_id: i64,
        command: NotebookArtifactCommand,
    ) -> Result<NotebookArtifactResp, AppError> {
        ensure_positive_id("Notebook Workspace ID", workspace_id)?;
        self.ensure_workspace(tenant_id, user_id, workspace_id)
            .await?;
        let command = normalize_artifact_command(command)?;
        let id = next_id();
        let now = Utc::now().naive_utc();
        let record = NotebookArtifactSaveRecord {
            id,
            tenant_id,
            workspace_id,
            artifact_kind: command.artifact_kind,
            title: command.title,
            content_json: command.content_json,
            content_text: command.content_text,
            citation_payload: command.citation_payload,
            source_trace_id: command.source_trace_id,
            metadata: command.metadata,
            status: NOTEBOOK_STATUS_ACTIVE,
            user_id,
            now,
        };
        self.repo.create_artifact(&record).await?;
        let row = self
            .repo
            .get_artifact(tenant_id, user_id, workspace_id, id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(NotebookArtifactResp::from(row))
    }

    pub async fn list_artifacts(
        &self,
        tenant_id: i64,
        user_id: i64,
        workspace_id: i64,
    ) -> Result<Vec<NotebookArtifactResp>, AppError> {
        ensure_positive_id("Notebook Workspace ID", workspace_id)?;
        self.ensure_workspace(tenant_id, user_id, workspace_id)
            .await?;
        let rows = self
            .repo
            .list_artifacts(tenant_id, user_id, workspace_id)
            .await?;
        Ok(rows.into_iter().map(NotebookArtifactResp::from).collect())
    }

    async fn ensure_workspace(
        &self,
        tenant_id: i64,
        user_id: i64,
        workspace_id: i64,
    ) -> Result<(), AppError> {
        self.repo
            .get_workspace(tenant_id, user_id, workspace_id)
            .await?
            .map(|_| ())
            .ok_or(AppError::NotFound)
    }
}

fn normalize_workspace_command(
    mut command: NotebookWorkspaceCommand,
) -> Result<NotebookWorkspaceCommand, AppError> {
    command.name = command.name.trim().to_owned();
    if command.name.is_empty() {
        return Err(AppError::bad_request("Notebook 名称不能为空"));
    }
    ensure_max_chars(
        "Notebook 名称",
        &command.name,
        NOTEBOOK_WORKSPACE_NAME_MAX_CHARS,
    )?;
    command.description = command.description.and_then(trim_to_none);
    if let Some(description) = &command.description {
        ensure_max_chars("Notebook 描述", description, NOTEBOOK_DESCRIPTION_MAX_CHARS)?;
    }
    command.metadata = normalize_json_object(command.metadata);
    Ok(command)
}

fn normalize_source_command(
    mut command: NotebookSourceCommand,
) -> Result<NotebookSourceCommand, AppError> {
    command.source_type = command.source_type.trim().to_ascii_lowercase();
    match command.source_type.as_str() {
        NOTEBOOK_SOURCE_TYPE_DATASET => {
            ensure_optional_positive_id("知识库 ID", command.knowledge_dataset_id)?
        }
        NOTEBOOK_SOURCE_TYPE_DOCUMENT => {
            ensure_optional_positive_id("知识库 ID", command.knowledge_dataset_id)?;
            ensure_optional_positive_id("文档 ID", command.knowledge_document_id)?;
            if command.knowledge_document_id.is_none() {
                return Err(AppError::bad_request("文档来源必须提供文档 ID"));
            }
        }
        _ => {
            return Err(AppError::bad_request(
                "Notebook 来源类型仅支持 dataset 或 document",
            ));
        }
    }
    if command.source_type == NOTEBOOK_SOURCE_TYPE_DATASET && command.knowledge_dataset_id.is_none()
    {
        return Err(AppError::bad_request("知识库来源必须提供知识库 ID"));
    }
    command.title = command.title.trim().to_owned();
    if command.title.is_empty() {
        return Err(AppError::bad_request("Notebook 来源标题不能为空"));
    }
    ensure_max_chars(
        "Notebook 来源标题",
        &command.title,
        NOTEBOOK_TITLE_MAX_CHARS,
    )?;
    command.citation_metadata = normalize_json_object(command.citation_metadata);
    command.metadata = normalize_json_object(command.metadata);
    Ok(command)
}

fn normalize_artifact_command(
    mut command: NotebookArtifactCommand,
) -> Result<NotebookArtifactCommand, AppError> {
    command.artifact_kind = command.artifact_kind.trim().to_ascii_lowercase();
    if command.artifact_kind.is_empty() {
        return Err(AppError::bad_request("Notebook Artifact 类型不能为空"));
    }
    ensure_max_chars(
        "Notebook Artifact 类型",
        &command.artifact_kind,
        NOTEBOOK_KIND_MAX_CHARS,
    )?;
    command.title = command.title.trim().to_owned();
    if command.title.is_empty() {
        return Err(AppError::bad_request("Notebook Artifact 标题不能为空"));
    }
    ensure_max_chars(
        "Notebook Artifact 标题",
        &command.title,
        NOTEBOOK_TITLE_MAX_CHARS,
    )?;
    command.source_trace_id = command.source_trace_id.and_then(trim_to_none);
    command.content_json = normalize_json_object(command.content_json);
    command.citation_payload = normalize_json_array(command.citation_payload);
    command.metadata = normalize_json_object(command.metadata);
    Ok(command)
}

fn normalize_json_object(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        json!({})
    }
}

fn normalize_json_array(value: Value) -> Value {
    if value.is_array() {
        value
    } else {
        json!([])
    }
}

fn ensure_positive_id(label: &str, value: i64) -> Result<(), AppError> {
    if value <= 0 {
        return Err(AppError::bad_request(format!("{label} 不合法")));
    }
    Ok(())
}

fn ensure_optional_positive_id(label: &str, value: Option<i64>) -> Result<(), AppError> {
    if let Some(value) = value {
        ensure_positive_id(label, value)?;
    }
    Ok(())
}

impl From<NotebookWorkspaceRow> for NotebookWorkspaceResp {
    fn from(row: NotebookWorkspaceRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            owner_id: row.owner_id,
            name: row.name,
            description: row.description,
            metadata: row.metadata,
            status: row.status,
            create_user: row.create_user,
            create_time: format_datetime(row.create_time),
            update_time: format_optional_datetime(row.update_time),
        }
    }
}

impl From<NotebookSourceRow> for NotebookSourceResp {
    fn from(row: NotebookSourceRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            workspace_id: row.workspace_id,
            source_type: row.source_type,
            knowledge_dataset_id: row.knowledge_dataset_id,
            knowledge_document_id: row.knowledge_document_id,
            title: row.title,
            citation_metadata: row.citation_metadata,
            metadata: row.metadata,
            status: row.status,
            create_user: row.create_user,
            create_time: format_datetime(row.create_time),
            update_time: format_optional_datetime(row.update_time),
        }
    }
}

impl From<NotebookArtifactRow> for NotebookArtifactResp {
    fn from(row: NotebookArtifactRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            workspace_id: row.workspace_id,
            artifact_kind: row.artifact_kind,
            title: row.title,
            content_json: row.content_json,
            content_text: row.content_text,
            citation_payload: row.citation_payload,
            source_trace_id: row.source_trace_id,
            metadata: row.metadata,
            status: row.status,
            create_user: row.create_user,
            create_time: format_datetime(row.create_time),
            update_time: format_optional_datetime(row.update_time),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notebook_migration_defines_workspace_source_and_artifact_tables() {
        let migration =
            include_str!("../../../migrations/202606160003_create_ai_notebook_workspace.sql");

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_notebook_workspace"));
        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_notebook_source"));
        assert!(migration.contains("CREATE TABLE IF NOT EXISTS ai_notebook_artifact"));
        assert!(migration.contains("knowledge_dataset_id"));
        assert!(migration.contains("citation_payload"));
    }

    #[test]
    fn notebook_source_command_requires_existing_knowledge_reference() {
        let err = normalize_source_command(NotebookSourceCommand {
            source_type: "dataset".to_owned(),
            title: "Q2 Handbook".to_owned(),
            ..Default::default()
        })
        .unwrap_err();

        assert!(err.to_string().contains("知识库来源必须提供知识库 ID"));
    }

    #[test]
    fn notebook_artifact_command_normalizes_citation_payload_to_array() {
        let command = normalize_artifact_command(NotebookArtifactCommand {
            artifact_kind: " Summary ".to_owned(),
            title: " Brief ".to_owned(),
            citation_payload: Value::Null,
            metadata: Value::Null,
            ..Default::default()
        })
        .expect("artifact command should normalize");

        assert_eq!(command.artifact_kind, "summary");
        assert_eq!(command.title, "Brief");
        assert_eq!(command.citation_payload, json!([]));
        assert_eq!(command.metadata, json!({}));
    }
}
