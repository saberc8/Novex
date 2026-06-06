use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::{
        ai::{
            knowledge_service::{CitationResp, KnowledgeService, RagAskCommand},
            model_service::{
                ModelChatCommand, ModelChatFileContext, ModelChatMessage, ModelRuntimeService,
            },
        },
        system::{ensure_max_chars, format_datetime, format_optional_datetime},
    },
    infrastructure::persistence::{
        ai_chat_flow_repository::{
            AiChatFlowRepository, ChatFlowMessageRow, ChatFlowMessageSaveRecord,
            ChatFlowSessionFilter, ChatFlowSessionRow, ChatFlowSessionSaveRecord,
            ChatFlowSessionUpdateRecord,
        },
        ai_knowledge_repository::AiKnowledgeRepository,
    },
    shared::{error::AppError, id::next_id},
};

const CHAT_FLOW_APP_CODE: &str = "chat-web";
const CHAT_FLOW_MODE_KNOWLEDGE: &str = "knowledge";
const CHAT_FLOW_MODE_MODEL: &str = "model";
const CHAT_FLOW_SESSION_STATUS_ACTIVE: i16 = 1;
const DEFAULT_RAG_LIMIT: usize = 5;
const MAX_RAG_LIMIT: usize = 10;
const MAX_MESSAGE_CHARS: usize = 12_000;
const SESSION_TITLE_CHARS: usize = 60;
const MESSAGE_PREVIEW_CHARS: usize = 160;
const RAG_ROUTE_ID: &str = "novex-rag";

#[derive(Debug, Clone)]
pub struct ChatFlowService {
    repo: AiChatFlowRepository,
    knowledge_repo: AiKnowledgeRepository,
    db: PgPool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFlowSessionQuery {
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFlowSessionCommand {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub dataset_id: Option<i64>,
    #[serde(default)]
    pub title: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFlowMessageCommand {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub file_contexts: Vec<ModelChatFileContext>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default, rename = "maxTokens")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFlowSessionResp {
    pub id: i64,
    pub tenant_id: i64,
    pub app_code: String,
    pub mode: String,
    pub dataset_id: Option<i64>,
    pub title: String,
    pub status: i16,
    pub route_id: Option<String>,
    pub model: Option<String>,
    pub message_count: i32,
    pub last_message_preview: String,
    pub metadata: Value,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFlowMessageResp {
    pub id: i64,
    pub tenant_id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub route_id: Option<String>,
    pub model: Option<String>,
    pub rag_trace_id: Option<i64>,
    pub citations: Vec<CitationResp>,
    pub token_count: i32,
    pub metadata: Value,
    pub create_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFlowSendMessageResp {
    pub session: ChatFlowSessionResp,
    pub user_message: ChatFlowMessageResp,
    pub assistant_message: ChatFlowMessageResp,
}

impl ChatFlowService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiChatFlowRepository::new(db.clone()),
            knowledge_repo: AiKnowledgeRepository::new(db.clone()),
            db,
        }
    }

    pub async fn create_session(
        &self,
        tenant_id: i64,
        user_id: i64,
        command: ChatFlowSessionCommand,
    ) -> Result<ChatFlowSessionResp, AppError> {
        let command = normalize_chat_flow_session_command(command)?;
        if command.mode == CHAT_FLOW_MODE_KNOWLEDGE {
            let dataset_id = command
                .dataset_id
                .ok_or_else(|| AppError::bad_request("知识库会话必须选择知识库"))?;
            if !self
                .knowledge_repo
                .dataset_exists(tenant_id, dataset_id)
                .await?
            {
                return Err(AppError::NotFound);
            }
        }

        let session_id = next_id();
        let now = Utc::now().naive_utc();
        let record = ChatFlowSessionSaveRecord {
            id: session_id,
            tenant_id,
            app_code: CHAT_FLOW_APP_CODE.to_owned(),
            mode: command.mode.clone(),
            dataset_id: command.dataset_id,
            title: chat_flow_session_title(&command),
            status: CHAT_FLOW_SESSION_STATUS_ACTIVE,
            route_id: None,
            model: None,
            metadata: json!({
                "source": "ai.chatFlow",
                "mode": command.mode,
                "datasetId": command.dataset_id,
            }),
            user_id,
            now,
        };
        self.repo.create_session(&record).await?;

        let row = self
            .repo
            .get_session(tenant_id, user_id, session_id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(ChatFlowSessionResp::from(row))
    }

    pub async fn list_sessions(
        &self,
        tenant_id: i64,
        user_id: i64,
        query: ChatFlowSessionQuery,
    ) -> Result<Vec<ChatFlowSessionResp>, AppError> {
        let mode = normalize_optional_mode(query.mode)?;
        let rows = self
            .repo
            .list_sessions(&ChatFlowSessionFilter {
                tenant_id,
                user_id,
                mode: mode.as_deref(),
                limit: 50,
            })
            .await?;
        Ok(rows.into_iter().map(ChatFlowSessionResp::from).collect())
    }

    pub async fn list_messages(
        &self,
        tenant_id: i64,
        user_id: i64,
        session_id: i64,
    ) -> Result<Vec<ChatFlowMessageResp>, AppError> {
        ensure_session_id(session_id)?;
        let messages = self
            .repo
            .list_messages(tenant_id, user_id, session_id)
            .await?;
        Ok(messages.into_iter().map(ChatFlowMessageResp::from).collect())
    }

    pub async fn send_message(
        &self,
        tenant_id: i64,
        user_id: i64,
        session_id: i64,
        command: ChatFlowMessageCommand,
    ) -> Result<ChatFlowSendMessageResp, AppError> {
        ensure_session_id(session_id)?;
        let command = normalize_chat_flow_message_command(command)?;
        let session = self
            .repo
            .get_session(tenant_id, user_id, session_id)
            .await?
            .ok_or(AppError::NotFound)?;

        match session.mode.as_str() {
            CHAT_FLOW_MODE_KNOWLEDGE => {
                self.send_knowledge_message(tenant_id, user_id, session, command)
                    .await
            }
            CHAT_FLOW_MODE_MODEL => {
                self.send_model_message(tenant_id, user_id, session, command)
                    .await
            }
            _ => Err(AppError::bad_request("会话模式不支持")),
        }
    }

    async fn send_knowledge_message(
        &self,
        tenant_id: i64,
        user_id: i64,
        session: ChatFlowSessionRow,
        command: ChatFlowMessageCommand,
    ) -> Result<ChatFlowSendMessageResp, AppError> {
        let dataset_id = session
            .dataset_id
            .ok_or_else(|| AppError::bad_request("知识库会话缺少知识库"))?;
        let knowledge_service = KnowledgeService::new(self.db.clone());
        let rag = knowledge_service
            .ask_dataset_for_tenant(
                tenant_id,
                user_id,
                dataset_id,
                RagAskCommand {
                    question: command.content.clone(),
                    limit: command.limit.unwrap_or(DEFAULT_RAG_LIMIT),
                },
            )
            .await?;
        let now = Utc::now().naive_utc();
        let user_message = user_chat_flow_message(
            tenant_id,
            user_id,
            session.id,
            &command.content,
            json!({
                "source": "ai.chatFlow.knowledge",
                "datasetId": dataset_id,
            }),
            now,
        );
        let citations = serde_json::to_value(&rag.citations).unwrap_or_else(|_| json!([]));
        let assistant_message = ChatFlowMessageSaveRecord {
            id: next_id(),
            tenant_id,
            session_id: session.id,
            role: "assistant".to_owned(),
            content: rag.answer.clone(),
            route_id: Some(RAG_ROUTE_ID.to_owned()),
            model: None,
            rag_trace_id: Some(rag.trace_id),
            citations,
            token_count: tokenish_count(&rag.answer),
            metadata: json!({
                "source": "ai.chatFlow.knowledge",
                "datasetId": dataset_id,
                "ragTraceId": rag.trace_id,
                "answerStrategy": rag.answer_strategy,
                "retrievalHitCount": rag.retrieval_hit_count,
            }),
            user_id,
            now,
        };
        self.persist_turn_and_response(
            tenant_id,
            user_id,
            session.id,
            Some(RAG_ROUTE_ID.to_owned()),
            None,
            &rag.answer,
            user_message,
            assistant_message,
            now,
        )
        .await
    }

    async fn send_model_message(
        &self,
        tenant_id: i64,
        user_id: i64,
        session: ChatFlowSessionRow,
        command: ChatFlowMessageCommand,
    ) -> Result<ChatFlowSendMessageResp, AppError> {
        let response = ModelRuntimeService::chat_completion(ModelChatCommand {
            messages: vec![ModelChatMessage {
                role: "user".to_owned(),
                content: command.content.clone(),
            }],
            file_contexts: command.file_contexts.clone(),
            temperature: command.temperature,
            max_tokens: command.max_tokens,
            ..ModelChatCommand::default()
        })
        .await?;
        let now = Utc::now().naive_utc();
        let user_message = user_chat_flow_message(
            tenant_id,
            user_id,
            session.id,
            &command.content,
            json!({
                "source": "ai.chatFlow.model",
                "fileContexts": file_context_metadata(&command.file_contexts),
            }),
            now,
        );
        let assistant_message = ChatFlowMessageSaveRecord {
            id: next_id(),
            tenant_id,
            session_id: session.id,
            role: "assistant".to_owned(),
            content: response.answer.clone(),
            route_id: Some(response.route_id.clone()),
            model: response.model.clone(),
            rag_trace_id: None,
            citations: json!([]),
            token_count: response
                .usage
                .completion_tokens
                .unwrap_or_else(|| tokenish_count(&response.answer) as i64)
                .max(0)
                .min(i32::MAX as i64) as i32,
            metadata: json!({
                "source": "ai.chatFlow.model",
                "latencyMs": u128_to_i64(response.latency_ms),
                "usage": response.usage,
            }),
            user_id,
            now,
        };
        self.persist_turn_and_response(
            tenant_id,
            user_id,
            session.id,
            Some(response.route_id),
            response.model,
            &response.answer,
            user_message,
            assistant_message,
            now,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn persist_turn_and_response(
        &self,
        tenant_id: i64,
        user_id: i64,
        session_id: i64,
        route_id: Option<String>,
        model: Option<String>,
        answer: &str,
        user_message: ChatFlowMessageSaveRecord,
        assistant_message: ChatFlowMessageSaveRecord,
        now: NaiveDateTime,
    ) -> Result<ChatFlowSendMessageResp, AppError> {
        let update = ChatFlowSessionUpdateRecord {
            tenant_id,
            session_id,
            route_id,
            model,
            message_count_increment: 2,
            last_message_preview: preview_chars(answer, MESSAGE_PREVIEW_CHARS),
            user_id,
            now,
        };
        let user_resp = ChatFlowMessageResp::from_save_record(&user_message);
        let assistant_resp = ChatFlowMessageResp::from_save_record(&assistant_message);
        self.repo
            .append_turn(&update, &[user_message, assistant_message])
            .await?;
        let session = self
            .repo
            .get_session(tenant_id, user_id, session_id)
            .await?
            .ok_or(AppError::NotFound)?;

        Ok(ChatFlowSendMessageResp {
            session: ChatFlowSessionResp::from(session),
            user_message: user_resp,
            assistant_message: assistant_resp,
        })
    }
}

fn normalize_chat_flow_session_command(
    mut command: ChatFlowSessionCommand,
) -> Result<ChatFlowSessionCommand, AppError> {
    command.mode = normalize_mode(command.mode)?;
    command.title = command.title.trim().to_owned();
    ensure_max_chars("会话标题", &command.title, 160)?;
    if matches!(command.dataset_id, Some(value) if value <= 0) {
        return Err(AppError::bad_request("知识库 ID 不合法"));
    }
    if command.mode == CHAT_FLOW_MODE_KNOWLEDGE && command.dataset_id.is_none() {
        return Err(AppError::bad_request("知识库会话必须选择知识库"));
    }
    if command.mode == CHAT_FLOW_MODE_MODEL {
        command.dataset_id = None;
    }
    Ok(command)
}

fn normalize_chat_flow_message_command(
    mut command: ChatFlowMessageCommand,
) -> Result<ChatFlowMessageCommand, AppError> {
    command.content = command.content.trim().to_owned();
    if command.content.is_empty() {
        return Err(AppError::bad_request("消息内容不能为空"));
    }
    ensure_max_chars("消息内容", &command.content, MAX_MESSAGE_CHARS)?;
    command.limit = Some(
        command
            .limit
            .unwrap_or(DEFAULT_RAG_LIMIT)
            .clamp(1, MAX_RAG_LIMIT),
    );
    Ok(command)
}

fn normalize_optional_mode(mode: Option<String>) -> Result<Option<String>, AppError> {
    mode.map(normalize_mode).transpose()
}

fn normalize_mode(mode: String) -> Result<String, AppError> {
    let mode = mode.trim().to_ascii_lowercase();
    if matches!(mode.as_str(), CHAT_FLOW_MODE_KNOWLEDGE | CHAT_FLOW_MODE_MODEL) {
        Ok(mode)
    } else {
        Err(AppError::bad_request("会话模式不支持"))
    }
}

fn ensure_session_id(session_id: i64) -> Result<(), AppError> {
    if session_id <= 0 {
        Err(AppError::bad_request("会话 ID 不合法"))
    } else {
        Ok(())
    }
}

fn chat_flow_session_title(command: &ChatFlowSessionCommand) -> String {
    if !command.title.is_empty() {
        preview_chars(&command.title, SESSION_TITLE_CHARS)
    } else if command.mode == CHAT_FLOW_MODE_KNOWLEDGE {
        "知识库对话".to_owned()
    } else {
        "模型对话".to_owned()
    }
}

fn user_chat_flow_message(
    tenant_id: i64,
    user_id: i64,
    session_id: i64,
    content: &str,
    metadata: Value,
    now: NaiveDateTime,
) -> ChatFlowMessageSaveRecord {
    ChatFlowMessageSaveRecord {
        id: next_id(),
        tenant_id,
        session_id,
        role: "user".to_owned(),
        content: content.to_owned(),
        route_id: None,
        model: None,
        rag_trace_id: None,
        citations: json!([]),
        token_count: tokenish_count(content),
        metadata,
        user_id,
        now,
    }
}

fn file_context_metadata(files: &[ModelChatFileContext]) -> Vec<Value> {
    files
        .iter()
        .map(|file| {
            json!({
                "name": file.name,
                "contentType": file.content_type,
                "charCount": file.content.chars().count(),
            })
        })
        .collect()
}

fn tokenish_count(text: &str) -> i32 {
    text.split_whitespace()
        .count()
        .max((text.chars().count() / 4).max(1))
        .min(i32::MAX as usize) as i32
}

fn preview_chars(text: &str, limit: usize) -> String {
    let mut value = text.trim().chars().take(limit).collect::<String>();
    if text.trim().chars().count() > limit {
        value.push('…');
    }
    value
}

fn u128_to_i64(value: u128) -> i64 {
    value.min(i64::MAX as u128) as i64
}

impl From<ChatFlowSessionRow> for ChatFlowSessionResp {
    fn from(row: ChatFlowSessionRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            app_code: row.app_code,
            mode: row.mode,
            dataset_id: row.dataset_id,
            title: row.title,
            status: row.status,
            route_id: row.route_id,
            model: row.model,
            message_count: row.message_count,
            last_message_preview: row.last_message_preview,
            metadata: row.metadata,
            create_time: format_datetime(row.create_time),
            update_time: format_optional_datetime(row.update_time),
        }
    }
}

impl ChatFlowMessageResp {
    fn from_save_record(record: &ChatFlowMessageSaveRecord) -> Self {
        Self {
            id: record.id,
            tenant_id: record.tenant_id,
            session_id: record.session_id,
            role: record.role.clone(),
            content: record.content.clone(),
            route_id: record.route_id.clone(),
            model: record.model.clone(),
            rag_trace_id: record.rag_trace_id,
            citations: chat_flow_citations(record.citations.clone()),
            token_count: record.token_count,
            metadata: record.metadata.clone(),
            create_time: format_datetime(record.now),
        }
    }
}

impl From<ChatFlowMessageRow> for ChatFlowMessageResp {
    fn from(row: ChatFlowMessageRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            session_id: row.session_id,
            role: row.role,
            content: row.content,
            route_id: row.route_id,
            model: row.model,
            rag_trace_id: row.rag_trace_id,
            citations: chat_flow_citations(row.citations),
            token_count: row.token_count,
            metadata: row.metadata,
            create_time: format_datetime(row.create_time),
        }
    }
}

fn chat_flow_citations(value: Value) -> Vec<CitationResp> {
    serde_json::from_value(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_session_requires_valid_knowledge_dataset() {
        let err = normalize_chat_flow_session_command(ChatFlowSessionCommand {
            mode: "knowledge".to_owned(),
            dataset_id: None,
            title: "Policy".to_owned(),
        })
        .unwrap_err();

        assert!(err.to_string().contains("知识库"));
    }

    #[test]
    fn send_message_trims_content_and_clamps_limit() {
        let command = normalize_chat_flow_message_command(ChatFlowMessageCommand {
            content: "  哪个制度有效？  ".to_owned(),
            limit: Some(50),
            ..ChatFlowMessageCommand::default()
        })
        .unwrap();

        assert_eq!(command.content, "哪个制度有效？");
        assert_eq!(command.limit, Some(10));
    }
}
