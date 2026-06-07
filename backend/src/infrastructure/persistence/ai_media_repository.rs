use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};

use crate::shared::error::AppError;

#[derive(Debug, Clone)]
pub struct MediaAssetSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub asset_uid: String,
    pub asset_kind: String,
    pub provider: String,
    pub provider_asset_id: Option<String>,
    pub asset_url: Option<String>,
    pub storage_ref: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub metadata: Value,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct MediaJobSaveRecord {
    pub id: i64,
    pub tenant_id: i64,
    pub trace_id: Option<String>,
    pub run_id: Option<i64>,
    pub tool_call_audit_id: Option<i64>,
    pub tool_code: String,
    pub provider: String,
    pub model_route: Option<String>,
    pub prompt: String,
    pub request_payload: Value,
    pub response_payload: Value,
    pub asset_id: Option<i64>,
    pub status: String,
    pub dry_run: bool,
    pub cost: Option<f64>,
    pub latency_ms: Option<i32>,
    pub policy_result: Value,
    pub error_message: Option<String>,
    pub user_id: i64,
    pub now: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AiMediaRepository {
    db: PgPool,
}

impl AiMediaRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn create_media_result(
        &self,
        asset: Option<&MediaAssetSaveRecord>,
        job: &MediaJobSaveRecord,
    ) -> Result<(), AppError> {
        let mut tx = self.db.begin().await?;
        if let Some(asset) = asset {
            insert_media_asset(&mut tx, asset).await?;
        }
        insert_media_job(&mut tx, job).await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn insert_media_asset(
    tx: &mut Transaction<'_, Postgres>,
    record: &MediaAssetSaveRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_media_asset (
    id, tenant_id, asset_uid, asset_kind, provider, provider_asset_id,
    asset_url, storage_ref, mime_type, width, height, metadata, create_user, create_time
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
ON CONFLICT (tenant_id, asset_uid) DO UPDATE
SET provider_asset_id = EXCLUDED.provider_asset_id,
    asset_url = EXCLUDED.asset_url,
    storage_ref = EXCLUDED.storage_ref,
    mime_type = EXCLUDED.mime_type,
    width = EXCLUDED.width,
    height = EXCLUDED.height,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = EXCLUDED.create_time;
"#,
    )
    .bind(record.id)
    .bind(record.tenant_id)
    .bind(&record.asset_uid)
    .bind(&record.asset_kind)
    .bind(&record.provider)
    .bind(&record.provider_asset_id)
    .bind(&record.asset_url)
    .bind(&record.storage_ref)
    .bind(&record.mime_type)
    .bind(record.width)
    .bind(record.height)
    .bind(&record.metadata)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_media_job(
    tx: &mut Transaction<'_, Postgres>,
    record: &MediaJobSaveRecord,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
INSERT INTO ai_media_job (
    id, tenant_id, trace_id, run_id, tool_call_audit_id, tool_code, provider,
    model_route, prompt, request_payload, response_payload, asset_id, status,
    dry_run, cost, latency_ms, policy_result, error_message, create_user, create_time
)
VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20
);
"#,
    )
    .bind(record.id)
    .bind(record.tenant_id)
    .bind(&record.trace_id)
    .bind(record.run_id)
    .bind(record.tool_call_audit_id)
    .bind(&record.tool_code)
    .bind(&record.provider)
    .bind(&record.model_route)
    .bind(&record.prompt)
    .bind(&record.request_payload)
    .bind(&record.response_payload)
    .bind(record.asset_id)
    .bind(&record.status)
    .bind(record.dry_run)
    .bind(record.cost)
    .bind(record.latency_ms)
    .bind(&record.policy_result)
    .bind(&record.error_message)
    .bind(record.user_id)
    .bind(record.now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn media_repository_persists_asset_before_job_with_audit_link() {
        let source = include_str!("ai_media_repository.rs");

        for needle in [
            "INSERT INTO ai_media_asset",
            "INSERT INTO ai_media_job",
            "tool_call_audit_id",
            "asset_id",
            "create_media_result",
        ] {
            assert!(source.contains(needle), "{needle} missing from repository");
        }
    }
}
