mod http;
mod media;
mod native_cancel;
mod rag;

pub(super) use http::model_provider_client_error_to_app_error;
pub(super) use media::{send_model_provider_media_image_request, ModelProviderMediaImageRequest};
pub(super) use native_cancel::{
    send_model_provider_native_cancel_request, ModelProviderNativeCancelRequest,
};
pub(super) use novex_provider_client::{
    build_model_provider_chat_plan, model_chat_sse_record_data_payload,
    model_provider_chat_plan_streams_chat_completion, model_provider_response_id_from_payloads,
    normalize_model_provider_response_id, parse_model_provider_embedding_vectors,
    parse_model_provider_rerank_scores, ModelChatCompactionProviderOutput, ModelChatProviderOutput,
    ModelChatStreamCompletionBuilder, ModelProviderChatCompactionMetadata,
    ModelProviderChatFileContext, ModelProviderChatMessage, ModelProviderChatPlan,
    ModelProviderChatPlanInput, ModelProviderChatRequest, ModelProviderChatRequestKind,
    ModelProviderChatRequestMetadata, ModelProviderChatTransport,
};
use serde_json::Value;

pub(super) use rag::{
    send_model_provider_embedding_request, send_model_provider_rerank_request,
    ModelProviderEmbeddingRequest, ModelProviderRerankRequest,
};

use crate::shared::error::AppError;

#[allow(dead_code)]
pub(super) async fn read_model_provider_response_text(
    response: reqwest::Response,
) -> Result<String, AppError> {
    novex_provider_client::read_model_provider_response_text(response)
        .await
        .map_err(model_provider_client_error_to_app_error)
}

pub(super) async fn send_model_provider_chat_request(
    request: ModelProviderChatRequest<'_>,
) -> Result<reqwest::Response, AppError> {
    novex_provider_client::send_model_provider_chat_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}

pub(super) async fn send_model_provider_chat_unary_request(
    request: ModelProviderChatRequest<'_>,
) -> Result<String, AppError> {
    novex_provider_client::send_model_provider_chat_unary_request(request)
        .await
        .map_err(model_provider_client_error_to_app_error)
}

pub(super) fn parse_model_chat_provider_output_from_text(
    body_text: &str,
) -> Result<ModelChatProviderOutput, AppError> {
    novex_provider_client::parse_model_chat_provider_output_from_text(body_text)
        .map_err(model_provider_client_error_to_app_error)
}

pub(super) fn parse_model_chat_provider_output_from_body(
    body: &Value,
) -> Result<ModelChatProviderOutput, AppError> {
    novex_provider_client::parse_model_chat_provider_output_from_body(body)
        .map_err(model_provider_client_error_to_app_error)
}

pub(super) fn parse_model_chat_compaction_provider_output_from_text(
    body_text: &str,
) -> Result<ModelChatCompactionProviderOutput, AppError> {
    novex_provider_client::parse_model_chat_compaction_provider_output_from_text(body_text)
        .map_err(model_provider_client_error_to_app_error)
}

#[allow(dead_code)]
pub(super) fn parse_model_chat_compaction_provider_output_from_body(
    body: &Value,
) -> Result<ModelChatCompactionProviderOutput, AppError> {
    novex_provider_client::parse_model_chat_compaction_provider_output_from_body(body)
        .map_err(model_provider_client_error_to_app_error)
}

#[allow(dead_code)]
pub(super) fn parse_model_chat_compaction_provider_output_from_sse_text(
    body_text: &str,
) -> Result<ModelChatCompactionProviderOutput, AppError> {
    novex_provider_client::parse_model_chat_compaction_provider_output_from_sse_text(body_text)
        .map_err(model_provider_client_error_to_app_error)
}
