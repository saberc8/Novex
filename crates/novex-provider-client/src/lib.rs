mod chat_dispatch;
mod chat_parse;
mod chat_plan;
mod compaction;
mod error;
mod http;
mod media;
mod native_cancel;
mod rag;

pub use chat_dispatch::{
    send_model_provider_chat_request, send_model_provider_chat_unary_request,
    ModelProviderChatRequest,
};
pub(crate) use chat_parse::{
    model_chat_sse_data_payloads, model_provider_response_status_from_payload,
};
pub use chat_parse::{
    model_chat_sse_record_data_payload, model_provider_response_id_from_payload,
    model_provider_response_id_from_payloads, normalize_model_provider_response_id,
    parse_model_chat_provider_output_from_body, parse_model_chat_provider_output_from_sse_text,
    parse_model_chat_provider_output_from_text, ModelChatProviderOutput,
    ModelChatStreamCompletionBuilder,
};
pub use chat_plan::{
    build_model_provider_chat_plan, model_provider_chat_plan_streams_chat_completion,
    ModelProviderChatCompactionMetadata, ModelProviderChatFileContext, ModelProviderChatMessage,
    ModelProviderChatPlan, ModelProviderChatPlanInput, ModelProviderChatRequestKind,
    ModelProviderChatRequestMetadata, ModelProviderChatTransport,
};
pub use compaction::{
    parse_model_chat_compaction_provider_output_from_body,
    parse_model_chat_compaction_provider_output_from_sse_text,
    parse_model_chat_compaction_provider_output_from_text, ModelChatCompactionProviderOutput,
};
pub use error::ModelProviderClientError;
pub use http::{
    model_provider_http_client, read_model_provider_response_text,
    send_model_provider_http_request, ModelProviderHttpRequest,
};
pub use media::{send_model_provider_media_image_request, ModelProviderMediaImageRequest};
pub use native_cancel::{
    send_model_provider_native_cancel_request, ModelProviderNativeCancelRequest,
};
pub use rag::{
    parse_model_provider_embedding_vectors, parse_model_provider_rerank_scores,
    send_model_provider_embedding_request, send_model_provider_rerank_request,
    ModelProviderEmbeddingRequest, ModelProviderRerankRequest,
};

pub const CRATE_ID: &str = "novex-provider-client";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_describes_provider_client_boundary() {
        assert_eq!(CRATE_ID, "novex-provider-client");
    }
}
