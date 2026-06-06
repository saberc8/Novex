use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{FromRow, PgPool};

use crate::shared::error::AppError;

const CHAT_FLOW_SESSION_LIMIT: i64 = 50;

#[derive(Debug, Clone)]
pub struct AiChatFlowRepository {
    db: PgPool,
}

#[derive(Debug, Clone, FromRow)]
pub struct ChatFlowSessionRow {
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
    pub create_user: i64,
    pub create_time: NaiveDateTime,
    pub update_time: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ChatFlowMessageRow {
    pub id: i64,
    pub tenant_id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub route_id: Option<String>,
    pub model: Option<String>,
    pub rag_trace_id: Option<i64>,
    pub citations: Value,
    pub token_count: i32,
    pub metadata: Value,
    pub create_user: i64,
    pub create_time: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ChatFlowSessionFilter<'a> {
    pub tenant_id: i64,
    pub user_id: i64,
    pub mode: Option<&'a str>,
    pub limit: i64,
}

#[derive(Debug, Clone)]
pub struct ChatFlowSessionSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub app_code: String,
    pub mode: String,
    pub dataset_id: Option<i64>,
    pub title: String,
    pub status: i16,
    pub route_id: Option<String>,
    pub model: Option<String>,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ChatFlowSessionUpdateRecord {
    pub tenant_id: i64,
    pub session_id: i64,
    pub route_id: Option<String>,
    pub model: Option<String>,
    pub message_count_increment: i32,
    pub last_message_preview: String,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ChatFlowMessageSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub route_id: Option<String>,
    pub model: Option<String>,
    pub rag_trace_id: Option<i64>,
    pub citations: Value,
    pub token_count: i32,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

impl AiChatFlowRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn create_session(&self, record: &ChatFlowSessionSaveRecord) -> Result<(), AppError> {
        sqlx::query(
            r#"
INSERT INTO ai_chat_flow_session (
    id, tenant_id, app_code, mode, dataset_id, title, status, route_id, model,
    message_count, last_message_preview, metadata, create_user, create_time,
    update_user, update_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 0, '', $10, $11, $12, $11, $12);
"#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(&record.app_code)
        .bind(&record.mode)
        .bind(record.dataset_id)
        .bind(&record.title)
        .bind(record.status)
        .bind(&record.route_id)
        .bind(&record.model)
        .bind(&record.metadata)
        .bind(record.user_id)
        .bind(record.now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn list_sessions(
        &self,
        filter: &ChatFlowSessionFilter<'_>,
    ) -> Result<Vec<ChatFlowSessionRow>, AppError> {
        let limit = filter.limit.clamp(1, CHAT_FLOW_SESSION_LIMIT);
        let rows = if let Some(mode) = filter.mode {
            sqlx::query_as::<_, ChatFlowSessionRow>(
                r#"
SELECT
    id, tenant_id, app_code, mode, dataset_id, title, status, route_id, model,
    message_count, last_message_preview, metadata, create_user, create_time, update_time
FROM ai_chat_flow_session
WHERE tenant_id = $1
  AND create_user = $2
  AND mode = $3
ORDER BY COALESCE(update_time, create_time) DESC, id DESC
LIMIT $4;
"#,
            )
            .bind(filter.tenant_id)
            .bind(filter.user_id)
            .bind(mode)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query_as::<_, ChatFlowSessionRow>(
                r#"
SELECT
    id, tenant_id, app_code, mode, dataset_id, title, status, route_id, model,
    message_count, last_message_preview, metadata, create_user, create_time, update_time
FROM ai_chat_flow_session
WHERE tenant_id = $1
  AND create_user = $2
ORDER BY COALESCE(update_time, create_time) DESC, id DESC
LIMIT $3;
"#,
            )
            .bind(filter.tenant_id)
            .bind(filter.user_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        };
        Ok(rows)
    }

    pub async fn get_session(
        &self,
        tenant_id: i64,
        user_id: i64,
        session_id: i64,
    ) -> Result<Option<ChatFlowSessionRow>, AppError> {
        Ok(sqlx::query_as::<_, ChatFlowSessionRow>(
            r#"
SELECT
    id, tenant_id, app_code, mode, dataset_id, title, status, route_id, model,
    message_count, last_message_preview, metadata, create_user, create_time, update_time
FROM ai_chat_flow_session
WHERE tenant_id = $1
  AND create_user = $2
  AND id = $3;
"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(session_id)
        .fetch_optional(&self.db)
        .await?)
    }

    pub async fn list_messages(
        &self,
        tenant_id: i64,
        user_id: i64,
        session_id: i64,
    ) -> Result<Vec<ChatFlowMessageRow>, AppError> {
        Ok(sqlx::query_as::<_, ChatFlowMessageRow>(
            r#"
SELECT
    m.id, m.tenant_id, m.session_id, m.role, m.content, m.route_id, m.model,
    m.rag_trace_id, m.citations, m.token_count, m.metadata, m.create_user, m.create_time
FROM ai_chat_flow_message AS m
INNER JOIN ai_chat_flow_session AS s
    ON s.tenant_id = m.tenant_id
   AND s.id = m.session_id
WHERE m.tenant_id = $1
  AND s.create_user = $2
  AND m.session_id = $3
ORDER BY m.create_time ASC, m.id ASC;
"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(session_id)
        .fetch_all(&self.db)
        .await?)
    }

    pub async fn append_turn(
        &self,
        session: &ChatFlowSessionUpdateRecord,
        messages: &[ChatFlowMessageSaveRecord],
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        let result = sqlx::query(
            r#"
UPDATE ai_chat_flow_session
SET route_id = $4,
    model = $5,
    message_count = message_count + $6,
    last_message_preview = $7,
    update_user = $3,
    update_time = $8
WHERE tenant_id = $1
  AND id = $2
  AND create_user = $3;
"#,
        )
        .bind(session.tenant_id)
        .bind(session.session_id)
        .bind(session.user_id)
        .bind(&session.route_id)
        .bind(&session.model)
        .bind(session.message_count_increment)
        .bind(&session.last_message_preview)
        .bind(session.now)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }

        for message in messages {
            sqlx::query(
                r#"
INSERT INTO ai_chat_flow_message (
    id, tenant_id, session_id, role, content, route_id, model, rag_trace_id,
    citations, token_count, metadata, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13);
"#,
            )
            .bind(message.id)
            .bind(message.tenant_id)
            .bind(message.session_id)
            .bind(&message.role)
            .bind(&message.content)
            .bind(&message.route_id)
            .bind(&message.model)
            .bind(message.rag_trace_id)
            .bind(&message.citations)
            .bind(message.token_count)
            .bind(&message.metadata)
            .bind(message.user_id)
            .bind(message.now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_flow_message_record_keeps_rag_trace_and_citations() {
        let citations = serde_json::json!([
            {"documentId":"20","chunkId":"20:0","pageNo":3,"sectionPath":["Policy"]}
        ]);
        let record = ChatFlowMessageSaveRecord {
            id: 1,
            tenant_id: 1,
            session_id: 2,
            role: "assistant".to_owned(),
            content: "Use the handbook.".to_owned(),
            route_id: Some("local-extractive".to_owned()),
            model: None,
            rag_trace_id: Some(42),
            citations: citations.clone(),
            token_count: 3,
            metadata: serde_json::json!({"answerStrategy":"extractive"}),
            user_id: 7,
            now: chrono::NaiveDate::from_ymd_opt(2026, 6, 6)
                .unwrap()
                .and_hms_opt(1, 2, 3)
                .unwrap(),
        };

        assert_eq!(record.rag_trace_id, Some(42));
        assert_eq!(record.citations, citations);
    }
}
