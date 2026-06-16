use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::ai::knowledge_service::{
        CitationResp, KnowledgeService, RagAskCommand, RagAskResp,
    },
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
const NOTEBOOK_ASK_QUESTION_MAX_CHARS: usize = 2_000;
const DEFAULT_NOTEBOOK_ASK_LIMIT: usize = 6;
const MAX_NOTEBOOK_ASK_LIMIT: usize = 10;
const NOTEBOOK_SOURCE_TYPE_DATASET: &str = "dataset";
const NOTEBOOK_SOURCE_TYPE_DOCUMENT: &str = "document";
const NOTEBOOK_ARTIFACT_KIND_SUMMARY: &str = "summary";
const NOTEBOOK_ARTIFACT_KIND_FAQ: &str = "faq";
const NOTEBOOK_ARTIFACT_KIND_STUDY_GUIDE: &str = "study_guide";
const NOTEBOOK_ARTIFACT_KIND_NOTE: &str = "note";

#[derive(Debug, Clone)]
pub struct NotebookService {
    db: PgPool,
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookArtifactGenerateCommand {
    #[serde(default)]
    pub artifact_kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub generation_profile: Option<String>,
    #[serde(default)]
    pub answer_model_route_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookAskCommand {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub generation_profile: Option<String>,
    #[serde(default)]
    pub answer_model_route_id: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookFeedbackTargetResp {
    pub resource_type: String,
    pub resource_id: String,
    pub trace_id: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookAskResp {
    pub trace_id: i64,
    pub answer: String,
    pub citations: Vec<CitationResp>,
    pub retrieval_hit_count: usize,
    pub answer_strategy: String,
    pub embedding_model_route: String,
    pub rerank_model_route: String,
    pub answer_model_route: String,
    pub answer_model: Option<String>,
    pub source_ids: Vec<i64>,
    pub knowledge_dataset_ids: Vec<i64>,
    pub feedback_target: NotebookFeedbackTargetResp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NotebookSourceScope {
    dataset_id: i64,
    source_ids: Vec<i64>,
    document_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
struct NotebookScopedRagCommand {
    dataset_id: i64,
    command: RagAskCommand,
}

impl NotebookService {
    pub fn new(db: PgPool) -> Self {
        Self {
            db: db.clone(),
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

    pub async fn generate_artifact(
        &self,
        tenant_id: i64,
        user_id: i64,
        workspace_id: i64,
        command: NotebookArtifactGenerateCommand,
    ) -> Result<NotebookArtifactResp, AppError> {
        let command = normalize_artifact_generate_command(command)?;
        let ask_command = notebook_ask_command_for_artifact(&command);
        let ask_response = self
            .ask_workspace(tenant_id, user_id, workspace_id, ask_command)
            .await?;
        let artifact = notebook_artifact_command_from_ask(&command, &ask_response);
        self.create_artifact(tenant_id, user_id, workspace_id, artifact)
            .await
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

    pub async fn ask_workspace(
        &self,
        tenant_id: i64,
        user_id: i64,
        workspace_id: i64,
        command: NotebookAskCommand,
    ) -> Result<NotebookAskResp, AppError> {
        ensure_positive_id("Notebook Workspace ID", workspace_id)?;
        self.ensure_workspace(tenant_id, user_id, workspace_id)
            .await?;
        let command = normalize_ask_command(command)?;
        let sources = self
            .repo
            .list_sources(tenant_id, user_id, workspace_id)
            .await?;
        let scopes = notebook_source_scopes(&sources)?;
        if scopes.is_empty() {
            return Err(AppError::bad_request("Notebook 至少需要一个可检索来源"));
        }
        let source_ids = notebook_source_ids(&scopes);
        let dataset_ids = notebook_dataset_ids(&scopes);
        let rag_commands = notebook_rag_commands_for_scopes(&command, &scopes)?;
        let knowledge_service = KnowledgeService::new(self.db.clone());
        let mut responses = Vec::new();
        for scoped_command in rag_commands {
            let response = knowledge_service
                .ask_dataset_for_tenant(
                    tenant_id,
                    user_id,
                    scoped_command.dataset_id,
                    scoped_command.command,
                )
                .await?;
            responses.push(response);
        }
        let response = responses
            .into_iter()
            .max_by_key(|response| response.retrieval_hit_count)
            .ok_or_else(|| AppError::bad_request("Notebook 没有可用检索结果"))?;

        Ok(notebook_ask_response_from_rag(
            source_ids,
            dataset_ids,
            response,
        ))
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
            if command.knowledge_dataset_id.is_none() {
                return Err(AppError::bad_request("文档来源必须提供知识库 ID"));
            }
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

fn normalize_artifact_generate_command(
    mut command: NotebookArtifactGenerateCommand,
) -> Result<NotebookArtifactGenerateCommand, AppError> {
    command.artifact_kind = normalize_artifact_kind(command.artifact_kind)?;
    command.topic = command.topic.trim().to_owned();
    if command.topic.is_empty() {
        command.topic = "selected Notebook sources".to_owned();
    }
    ensure_max_chars(
        "Notebook Artifact 主题",
        &command.topic,
        NOTEBOOK_TITLE_MAX_CHARS,
    )?;
    command.title = command.title.trim().to_owned();
    if command.title.is_empty() {
        command.title = default_artifact_title(&command.artifact_kind, &command.topic);
    }
    ensure_max_chars(
        "Notebook Artifact 标题",
        &command.title,
        NOTEBOOK_TITLE_MAX_CHARS,
    )?;
    command.limit = Some(
        command
            .limit
            .and_then(|limit| usize::try_from(limit).ok())
            .filter(|limit| *limit > 0)
            .unwrap_or(DEFAULT_NOTEBOOK_ASK_LIMIT)
            .min(MAX_NOTEBOOK_ASK_LIMIT) as u64,
    );
    command.generation_profile = command.generation_profile.and_then(trim_to_none);
    if let Some(generation_profile) = &command.generation_profile {
        ensure_max_chars("Notebook 生成配置", generation_profile, 128)?;
    }
    command.answer_model_route_id = command.answer_model_route_id.and_then(trim_to_none);
    if let Some(route_id) = &command.answer_model_route_id {
        ensure_max_chars("模型路由", route_id, 128)?;
    }
    Ok(command)
}

fn normalize_artifact_kind(kind: String) -> Result<String, AppError> {
    let kind = kind.trim().replace('-', "_").to_ascii_lowercase();
    if matches!(
        kind.as_str(),
        NOTEBOOK_ARTIFACT_KIND_SUMMARY
            | NOTEBOOK_ARTIFACT_KIND_FAQ
            | NOTEBOOK_ARTIFACT_KIND_STUDY_GUIDE
            | NOTEBOOK_ARTIFACT_KIND_NOTE
    ) {
        Ok(kind)
    } else {
        Err(AppError::bad_request(
            "Notebook Artifact 类型仅支持 summary、faq、study_guide、note",
        ))
    }
}

fn default_artifact_title(kind: &str, topic: &str) -> String {
    format!("{}: {topic}", artifact_kind_label(kind))
}

fn artifact_kind_label(kind: &str) -> &'static str {
    match kind {
        NOTEBOOK_ARTIFACT_KIND_SUMMARY => "Summary",
        NOTEBOOK_ARTIFACT_KIND_FAQ => "FAQ",
        NOTEBOOK_ARTIFACT_KIND_STUDY_GUIDE => "Study guide",
        NOTEBOOK_ARTIFACT_KIND_NOTE => "Note",
        _ => "Artifact",
    }
}

fn normalize_ask_command(mut command: NotebookAskCommand) -> Result<NotebookAskCommand, AppError> {
    command.question = command.question.trim().to_owned();
    if command.question.is_empty() {
        return Err(AppError::bad_request("Notebook 问题不能为空"));
    }
    ensure_max_chars(
        "Notebook 问题",
        &command.question,
        NOTEBOOK_ASK_QUESTION_MAX_CHARS,
    )?;
    command.limit = Some(
        command
            .limit
            .and_then(|limit| usize::try_from(limit).ok())
            .filter(|limit| *limit > 0)
            .unwrap_or(DEFAULT_NOTEBOOK_ASK_LIMIT)
            .min(MAX_NOTEBOOK_ASK_LIMIT) as u64,
    );
    command.generation_profile = command.generation_profile.and_then(trim_to_none);
    if let Some(generation_profile) = &command.generation_profile {
        ensure_max_chars("Notebook 生成配置", generation_profile, 128)?;
    }
    command.answer_model_route_id = command.answer_model_route_id.and_then(trim_to_none);
    if let Some(route_id) = &command.answer_model_route_id {
        ensure_max_chars("模型路由", route_id, 128)?;
    }
    Ok(command)
}

fn notebook_source_scopes(
    sources: &[NotebookSourceRow],
) -> Result<Vec<NotebookSourceScope>, AppError> {
    #[derive(Default)]
    struct ScopeBuilder {
        source_ids: BTreeSet<i64>,
        document_ids: BTreeSet<i64>,
        whole_dataset: bool,
    }

    let mut builders = BTreeMap::<i64, ScopeBuilder>::new();
    for source in sources
        .iter()
        .filter(|source| source.status == NOTEBOOK_STATUS_ACTIVE)
    {
        let dataset_id = source
            .knowledge_dataset_id
            .filter(|dataset_id| *dataset_id > 0)
            .ok_or_else(|| AppError::bad_request("Notebook 来源缺少知识库 ID"))?;
        let builder = builders.entry(dataset_id).or_default();
        builder.source_ids.insert(source.id);
        match source.source_type.as_str() {
            NOTEBOOK_SOURCE_TYPE_DATASET => {
                builder.whole_dataset = true;
                builder.document_ids.clear();
            }
            NOTEBOOK_SOURCE_TYPE_DOCUMENT => {
                if !builder.whole_dataset {
                    let document_id = source
                        .knowledge_document_id
                        .filter(|document_id| *document_id > 0)
                        .ok_or_else(|| AppError::bad_request("Notebook 文档来源缺少文档 ID"))?;
                    builder.document_ids.insert(document_id);
                }
            }
            _ => return Err(AppError::bad_request("Notebook 来源类型不支持检索")),
        }
    }

    Ok(builders
        .into_iter()
        .map(|(dataset_id, builder)| NotebookSourceScope {
            dataset_id,
            source_ids: builder.source_ids.into_iter().collect(),
            document_ids: if builder.whole_dataset {
                Vec::new()
            } else {
                builder.document_ids.into_iter().collect()
            },
        })
        .collect())
}

fn notebook_rag_commands_for_scopes(
    command: &NotebookAskCommand,
    scopes: &[NotebookSourceScope],
) -> Result<Vec<NotebookScopedRagCommand>, AppError> {
    let question = command.question.trim();
    if question.is_empty() {
        return Err(AppError::bad_request("Notebook 问题不能为空"));
    }
    Ok(scopes
        .iter()
        .map(|scope| NotebookScopedRagCommand {
            dataset_id: scope.dataset_id,
            command: RagAskCommand {
                question: question.to_owned(),
                limit: command
                    .limit
                    .and_then(|limit| usize::try_from(limit).ok())
                    .filter(|limit| *limit > 0)
                    .unwrap_or(DEFAULT_NOTEBOOK_ASK_LIMIT)
                    .min(MAX_NOTEBOOK_ASK_LIMIT),
                answer_model_route_id: command.answer_model_route_id.clone(),
                answer_instruction: Some(notebook_answer_instruction(
                    command.generation_profile.as_deref(),
                )),
                source_document_ids: scope.document_ids.clone(),
            },
        })
        .collect())
}

fn notebook_answer_instruction(generation_profile: Option<&str>) -> String {
    let mut instruction = "Answer as a Notebook workspace assistant. Use only the selected Notebook sources, preserve source citation labels, and say what evidence is missing when the selected sources are insufficient.".to_owned();
    if let Some(profile) = generation_profile
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
    {
        instruction.push_str("\nNotebook generation profile: ");
        instruction.push_str(profile);
    }
    instruction
}

fn notebook_ask_command_for_artifact(
    command: &NotebookArtifactGenerateCommand,
) -> NotebookAskCommand {
    let kind_label = artifact_kind_label(&command.artifact_kind);
    NotebookAskCommand {
        question: format!(
            "Generate a {kind_label} from the selected Notebook sources about \"{}\". Use source citations for factual claims. If the selected Notebook sources do not contain enough evidence, state what evidence is missing.",
            command.topic
        ),
        limit: command.limit,
        generation_profile: Some(notebook_artifact_generation_profile(command)),
        answer_model_route_id: command.answer_model_route_id.clone(),
    }
}

fn notebook_artifact_generation_profile(command: &NotebookArtifactGenerateCommand) -> String {
    let mut profile = format!("artifactKind={}", command.artifact_kind);
    if let Some(extra) = command
        .generation_profile
        .as_deref()
        .map(str::trim)
        .filter(|extra| !extra.is_empty())
    {
        profile.push_str("; ");
        profile.push_str(extra);
    }
    profile
}

fn notebook_artifact_command_from_ask(
    command: &NotebookArtifactGenerateCommand,
    ask: &NotebookAskResp,
) -> NotebookArtifactCommand {
    let citation_payload = serde_json::to_value(&ask.citations).unwrap_or_else(|_| json!([]));
    let feedback_target = serde_json::to_value(&ask.feedback_target).unwrap_or_else(|_| json!({}));
    NotebookArtifactCommand {
        artifact_kind: command.artifact_kind.clone(),
        title: command.title.clone(),
        content_json: json!({
            "artifactKind": command.artifact_kind,
            "title": command.title,
            "topic": command.topic,
            "content": ask.answer,
            "citations": ask.citations,
            "sourceIds": ask.source_ids,
            "knowledgeDatasetIds": ask.knowledge_dataset_ids,
            "traceId": ask.trace_id,
            "feedbackTarget": feedback_target,
        }),
        content_text: ask.answer.clone(),
        citation_payload,
        source_trace_id: Some(ask.trace_id.to_string()),
        metadata: json!({
            "source": "notebook.artifact.generate",
            "artifactKind": command.artifact_kind,
            "retrievalHitCount": ask.retrieval_hit_count,
            "answerStrategy": ask.answer_strategy,
            "answerModelRoute": ask.answer_model_route,
            "answerModel": ask.answer_model,
        }),
    }
}

fn notebook_ask_response_from_rag(
    source_ids: Vec<i64>,
    knowledge_dataset_ids: Vec<i64>,
    rag: RagAskResp,
) -> NotebookAskResp {
    let trace_id = rag.trace_id;
    let feedback_metadata = json!({
        "source": "notebook.ask",
        "sourceIds": source_ids,
        "knowledgeDatasetIds": knowledge_dataset_ids,
    });
    NotebookAskResp {
        trace_id,
        answer: rag.answer,
        citations: rag.citations,
        retrieval_hit_count: rag.retrieval_hit_count,
        answer_strategy: rag.answer_strategy,
        embedding_model_route: rag.embedding_model_route,
        rerank_model_route: rag.rerank_model_route,
        answer_model_route: rag.answer_model_route,
        answer_model: rag.answer_model,
        source_ids,
        knowledge_dataset_ids,
        feedback_target: NotebookFeedbackTargetResp {
            resource_type: "rag_trace".to_owned(),
            resource_id: trace_id.to_string(),
            trace_id: Some(trace_id.to_string()),
            metadata: feedback_metadata,
        },
    }
}

fn notebook_source_ids(scopes: &[NotebookSourceScope]) -> Vec<i64> {
    scopes
        .iter()
        .flat_map(|scope| scope.source_ids.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn notebook_dataset_ids(scopes: &[NotebookSourceScope]) -> Vec<i64> {
    scopes
        .iter()
        .map(|scope| scope.dataset_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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

    #[test]
    fn notebook_ask_retrieves_only_workspace_sources() {
        let scopes = notebook_source_scopes(&[
            source_row(1, "document", Some(7), Some(21)),
            source_row(2, "document", Some(7), Some(22)),
            source_row(3, "document", Some(8), Some(99)),
        ])
        .expect("source scopes should build");

        let commands = notebook_rag_commands_for_scopes(
            &NotebookAskCommand {
                question: "How should support handle refunds?".to_owned(),
                limit: Some(6),
                generation_profile: Some("customer-support".to_owned()),
                answer_model_route_id: None,
            },
            &scopes,
        )
        .expect("rag commands should build");

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].dataset_id, 7);
        assert_eq!(commands[0].command.source_document_ids, vec![21, 22]);
        assert_eq!(commands[1].dataset_id, 8);
        assert_eq!(commands[1].command.source_document_ids, vec![99]);
    }

    #[test]
    fn notebook_answer_preserves_source_citation_labels() {
        let rag = rag_answer_with_citation(42, "Refund answers must cite the policy [1].");
        let response = notebook_ask_response_from_rag(vec![1, 2], vec![7], rag);

        assert_eq!(response.answer, "Refund answers must cite the policy [1].");
        assert_eq!(response.citations.len(), 1);
        assert_eq!(response.citations[0].document_id, "21");
        assert_eq!(response.citations[0].chunk_id, "policy:1");
    }

    #[test]
    fn notebook_ask_records_agent_trace_and_feedback_target() {
        let response =
            notebook_ask_response_from_rag(vec![1], vec![7], rag_answer_with_citation(42, "Done"));

        assert_eq!(response.trace_id, 42);
        assert_eq!(response.feedback_target.resource_type, "rag_trace");
        assert_eq!(response.feedback_target.resource_id, "42");
        assert_eq!(response.feedback_target.trace_id.as_deref(), Some("42"));
        assert_eq!(response.feedback_target.metadata["source"], "notebook.ask");
        assert_eq!(response.source_ids, vec![1]);
        assert_eq!(response.knowledge_dataset_ids, vec![7]);
    }

    #[test]
    fn notebook_artifact_command_accepts_summary_faq_and_study_guide() {
        for kind in ["summary", "faq", "study-guide", "study_guide", "note"] {
            let command = normalize_artifact_generate_command(NotebookArtifactGenerateCommand {
                artifact_kind: kind.to_owned(),
                title: " Support Pack ".to_owned(),
                topic: " refunds ".to_owned(),
                ..Default::default()
            })
            .expect("artifact kind should be accepted");

            assert!(!command.title.is_empty());
            assert!(matches!(
                command.artifact_kind.as_str(),
                "summary" | "faq" | "study_guide" | "note"
            ));
        }
    }

    #[test]
    fn notebook_artifact_generation_uses_workspace_sources() {
        let scopes = notebook_source_scopes(&[source_row(1, "document", Some(7), Some(21))])
            .expect("source scopes should build");
        let artifact = normalize_artifact_generate_command(NotebookArtifactGenerateCommand {
            artifact_kind: "faq".to_owned(),
            title: "Refund FAQ".to_owned(),
            topic: "refund handling".to_owned(),
            limit: Some(4),
            generation_profile: None,
            answer_model_route_id: None,
        })
        .expect("artifact command should normalize");
        let ask = notebook_ask_command_for_artifact(&artifact);
        let commands = notebook_rag_commands_for_scopes(&ask, &scopes)
            .expect("artifact rag commands should build");

        assert!(ask.question.contains("selected Notebook sources"));
        assert!(ask.question.contains("FAQ"));
        assert_eq!(commands[0].dataset_id, 7);
        assert_eq!(commands[0].command.source_document_ids, vec![21]);
    }

    #[test]
    fn notebook_artifact_records_citation_payload() {
        let artifact = normalize_artifact_generate_command(NotebookArtifactGenerateCommand {
            artifact_kind: "summary".to_owned(),
            title: "Refund Summary".to_owned(),
            topic: "refunds".to_owned(),
            ..Default::default()
        })
        .expect("artifact command should normalize");
        let ask_response = notebook_ask_response_from_rag(
            vec![1],
            vec![7],
            rag_answer_with_citation(42, "Refunds need approval [1]."),
        );
        let save = notebook_artifact_command_from_ask(&artifact, &ask_response);

        assert_eq!(save.artifact_kind, "summary");
        assert_eq!(save.content_text, "Refunds need approval [1].");
        assert_eq!(save.source_trace_id.as_deref(), Some("42"));
        assert_eq!(save.citation_payload[0]["documentId"], "21");
        assert_eq!(save.citation_payload[0]["chunkId"], "policy:1");
    }

    fn source_row(
        id: i64,
        source_type: &str,
        dataset_id: Option<i64>,
        document_id: Option<i64>,
    ) -> NotebookSourceRow {
        let now = Utc::now().naive_utc();
        NotebookSourceRow {
            id,
            tenant_id: 1,
            workspace_id: 10,
            source_type: source_type.to_owned(),
            knowledge_dataset_id: dataset_id,
            knowledge_document_id: document_id,
            title: format!("source-{id}"),
            citation_metadata: json!({}),
            metadata: json!({}),
            status: 1,
            create_user: 1,
            create_time: now,
            update_time: None,
        }
    }

    fn rag_answer_with_citation(trace_id: i64, answer: &str) -> RagAskResp {
        RagAskResp {
            trace_id,
            answer: answer.to_owned(),
            citations: vec![CitationResp {
                document_id: "21".to_owned(),
                chunk_id: "policy:1".to_owned(),
                page_no: Some(3),
                section_path: vec!["Refunds".to_owned()],
            }],
            retrieval_hit_count: 1,
            answer_strategy: "llm_grounded".to_owned(),
            embedding_model_route: "local-keyword".to_owned(),
            rerank_model_route: "none".to_owned(),
            answer_model_route: "local-extractive".to_owned(),
            answer_model: None,
        }
    }
}
