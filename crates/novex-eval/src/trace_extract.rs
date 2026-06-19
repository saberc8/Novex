use std::collections::BTreeSet;

use crate::case::{
    EvalCaseActual, EvalCaseCandidate, EvalCaseExpected, EvalMetricKind, EvalTargetKind,
    TraceEvalPolicy,
};
use novex_trace::{TraceBundle, TraceEventKind};
use serde_json::{json, Value};

impl EvalCaseCandidate {
    pub fn from_trace_bundle(bundle: &TraceBundle) -> Self {
        Self::from_trace_bundle_with_policy(bundle, TraceEvalPolicy::default())
    }

    pub fn from_trace_bundle_with_policy(bundle: &TraceBundle, policy: TraceEvalPolicy) -> Self {
        let prompt = trace_event_payload_text(bundle, TraceEventKind::UserMessage, "content")
            .unwrap_or_default();
        let tool_code = trace_event_payload_text(bundle, TraceEventKind::ToolCall, "toolCode");
        let final_answer =
            trace_last_event_payload_text(bundle, TraceEventKind::FinalAnswer, "answer");
        let answer_contains = final_answer
            .as_deref()
            .map(|answer| trace_answer_snippet(answer, policy.answer_snippet_max_chars))
            .filter(|answer| !answer.is_empty())
            .into_iter()
            .collect();
        let citations = trace_bundle_citations(bundle);
        let summary = bundle.replay_summary();
        let mut tags = serde_json::Map::new();
        tags.insert("source".to_owned(), json!("agent_trace"));
        tags.insert("traceId".to_owned(), json!(bundle.trace_id));
        tags.insert("toolCallCount".to_owned(), json!(summary.tool_call_count));
        tags.insert("finalStatus".to_owned(), json!(summary.final_status));
        tags.insert(
            "hasApprovalPause".to_owned(),
            json!(summary.has_approval_pause),
        );
        let guardian_review = trace_guardian_review_summary(bundle);
        if let Some(outcome) = guardian_review.outcome.as_deref() {
            tags.insert("guardianReviewOutcome".to_owned(), json!(outcome));
        }
        if let Some(source) = guardian_review.source.as_deref() {
            tags.insert("guardianReviewSource".to_owned(), json!(source));
        }
        if let Some(requires_human_approval) = guardian_review.requires_human_approval {
            tags.insert(
                "guardianReviewRequiresHumanApproval".to_owned(),
                json!(requires_human_approval),
            );
        }
        if let Some(status) = guardian_review.review_status.as_deref() {
            tags.insert("guardianReviewStatus".to_owned(), json!(status));
        }
        if let Some(failure_reason) = guardian_review.failure_reason.as_deref() {
            tags.insert(
                "guardianReviewFailureReason".to_owned(),
                json!(failure_reason),
            );
        }
        if let Some(route_id) = guardian_review.model_route_id.as_deref() {
            tags.insert("guardianReviewModelRouteId".to_owned(), json!(route_id));
        }
        if let Some(auto_approved) = guardian_review.auto_approved {
            tags.insert("guardianAutoApproved".to_owned(), json!(auto_approved));
        }
        if let Some(tool_code) = tool_code.as_deref() {
            tags.insert("toolCode".to_owned(), json!(tool_code));
        }
        let retrieval_count = trace_event_count(bundle, TraceEventKind::Retrieval);
        let compaction_count = trace_event_count(bundle, TraceEventKind::ContextCompaction);
        let cancelled = trace_event_count(bundle, TraceEventKind::Cancellation) > 0;
        tags.insert("retrievalCount".to_owned(), json!(retrieval_count));
        tags.insert("compactionCount".to_owned(), json!(compaction_count));
        if compaction_count > 0 {
            let compaction_summary = trace_compaction_summary(bundle);
            tags.insert(
                "modelCompactionCount".to_owned(),
                json!(compaction_summary.model_count),
            );
            tags.insert(
                "compactionFallbackCount".to_owned(),
                json!(compaction_summary.fallback_count),
            );
            if let Some(status) = compaction_summary.status.as_deref() {
                tags.insert("compactionStatus".to_owned(), json!(status));
            }
            if compaction_summary.remote_count > 0 {
                tags.insert(
                    "remoteCompactionCount".to_owned(),
                    json!(compaction_summary.remote_count),
                );
            }
            if let Some(implementation) = compaction_summary.implementation.as_deref() {
                tags.insert("compactionImplementation".to_owned(), json!(implementation));
            }
        }
        tags.insert("cancelled".to_owned(), json!(cancelled));
        if let Some(cancel_reason) = trace_first_cancellation_reason(bundle) {
            tags.insert("cancelReason".to_owned(), json!(cancel_reason));
        }
        let runtime_supervisor = trace_runtime_supervisor_summary(bundle);
        if let Some(task_kind) = runtime_supervisor.task_kind.as_deref() {
            tags.insert("runtimeSupervisorTaskKind".to_owned(), json!(task_kind));
        }
        if let Some(signal_sent) = runtime_supervisor.cancel_signal_sent {
            tags.insert(
                "runtimeSupervisorCancelSignalSent".to_owned(),
                json!(signal_sent),
            );
        }
        if let Some(active_before_cancel) = runtime_supervisor.active_before_cancel {
            tags.insert(
                "runtimeSupervisorActiveBeforeCancel".to_owned(),
                json!(active_before_cancel),
            );
        }
        let tool_io_task_summary = trace_tool_io_task_summary(bundle);
        if tool_io_task_summary.count > 0 {
            tags.insert(
                "toolIoTaskCount".to_owned(),
                json!(tool_io_task_summary.count),
            );
            tags.insert(
                "parallelToolIoTaskCount".to_owned(),
                json!(tool_io_task_summary.parallel_count),
            );
            tags.insert(
                "serialToolIoTaskCount".to_owned(),
                json!(tool_io_task_summary.serial_count),
            );
            tags.insert(
                "cancelledToolIoTaskCount".to_owned(),
                json!(tool_io_task_summary.cancelled_count),
            );
            tags.insert(
                "timeoutToolIoTaskCount".to_owned(),
                json!(tool_io_task_summary.timeout_count),
            );
            tags.insert(
                "toolIoTaskMaxDurationMs".to_owned(),
                json!(tool_io_task_summary.max_duration_ms),
            );
            tags.insert(
                "toolIoTaskSupervisors".to_owned(),
                json!(tool_io_task_summary
                    .supervisors
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()),
            );
        }
        let inference_summary = trace_inference_summary(bundle);
        tags.insert("inferenceCount".to_owned(), json!(inference_summary.count));
        if inference_summary.error_count > 0 {
            tags.insert(
                "inferenceErrorCount".to_owned(),
                json!(inference_summary.error_count),
            );
            tags.insert(
                "retryableInferenceErrorCount".to_owned(),
                json!(inference_summary.retryable_error_count),
            );
        }
        if inference_summary.retry_count > 0 {
            tags.insert(
                "modelRetryCount".to_owned(),
                json!(inference_summary.retry_count),
            );
        }
        if inference_summary.provider_attempt_count > 0 {
            tags.insert(
                "modelProviderAttemptCount".to_owned(),
                json!(inference_summary.provider_attempt_count),
            );
        }
        if inference_summary.delta_count > 0 {
            tags.insert(
                "modelDeltaCount".to_owned(),
                json!(inference_summary.delta_count),
            );
            tags.insert(
                "modelDeltaTextLength".to_owned(),
                json!(inference_summary.delta_text_length),
            );
            tags.insert("streamingModelOutput".to_owned(), json!(true));
        }
        if inference_summary.streaming_tool_call_count > 0 {
            tags.insert("streamingToolCallDetected".to_owned(), json!(true));
            tags.insert(
                "streamingToolCallCount".to_owned(),
                json!(inference_summary.streaming_tool_call_count),
            );
            tags.insert(
                "streamingToolCodes".to_owned(),
                json!(inference_summary
                    .streaming_tool_codes
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()),
            );
        }
        if inference_summary.provider_native_cancel_count > 0 {
            tags.insert(
                "providerNativeCancelCount".to_owned(),
                json!(inference_summary.provider_native_cancel_count),
            );
            if let Some(attempted) = inference_summary.provider_native_cancel_attempted {
                tags.insert("providerNativeCancelAttempted".to_owned(), json!(attempted));
            }
            if let Some(supported) = inference_summary.provider_native_cancel_supported {
                tags.insert("providerNativeCancelSupported".to_owned(), json!(supported));
            }
            if let Some(provider) = inference_summary.provider_native_cancel_provider.as_deref() {
                tags.insert("providerNativeCancelProvider".to_owned(), json!(provider));
            }
            if let Some(status) = inference_summary.provider_native_cancel_status.as_deref() {
                tags.insert("providerNativeCancelStatus".to_owned(), json!(status));
            }
            if let Some(http_status) = inference_summary.provider_native_cancel_http_status {
                tags.insert(
                    "providerNativeCancelHttpStatus".to_owned(),
                    json!(http_status),
                );
            }
        }
        if inference_summary.fallback_count > 0 {
            tags.insert(
                "modelFallbackCount".to_owned(),
                json!(inference_summary.fallback_count),
            );
            if let Some(route_id) = inference_summary.fallback_route_id.as_deref() {
                tags.insert("modelFallbackRouteId".to_owned(), json!(route_id));
            }
        }
        if inference_summary.circuit_open_count > 0 {
            tags.insert(
                "modelCircuitOpenCount".to_owned(),
                json!(inference_summary.circuit_open_count),
            );
        }
        if let Some(route_id) = inference_summary.route_id.as_deref() {
            tags.insert("modelRouteId".to_owned(), json!(route_id));
        }
        if let Some(provider) = inference_summary.provider.as_deref() {
            tags.insert("modelProvider".to_owned(), json!(provider));
        }
        if let Some(model) = inference_summary.model.as_deref() {
            tags.insert("modelName".to_owned(), json!(model));
        }
        if let Some(error_kind) = inference_summary.error_kind.as_deref() {
            tags.insert("modelErrorKind".to_owned(), json!(error_kind));
        }
        if let Some(http_status) = inference_summary.http_status {
            tags.insert("modelHttpStatus".to_owned(), json!(http_status));
        }
        if inference_summary.count > 0 {
            tags.insert(
                "promptTokens".to_owned(),
                json!(inference_summary.prompt_tokens),
            );
            tags.insert(
                "completionTokens".to_owned(),
                json!(inference_summary.completion_tokens),
            );
            tags.insert(
                "totalTokens".to_owned(),
                json!(inference_summary.total_tokens),
            );
        }
        if policy.include_latency_cost_tags {
            tags.insert(
                "latencyMs".to_owned(),
                if inference_summary.count > 0 {
                    json!(inference_summary.latency_ms)
                } else {
                    Value::Null
                },
            );
            tags.insert(
                "costCents".to_owned(),
                inference_summary
                    .cost_cents
                    .map(|cost_cents| json!(cost_cents))
                    .unwrap_or(Value::Null),
            );
        }

        Self {
            target_kind: EvalTargetKind::ReAct,
            metric_kind: if tool_code.is_some() {
                EvalMetricKind::ToolAccuracy
            } else {
                EvalMetricKind::Faithfulness
            },
            prompt,
            expected: EvalCaseExpected {
                answer_contains,
                citations,
                intent: None,
                tool_code,
            },
            tags: Value::Object(tags),
        }
    }
}

pub fn actual_from_trace_bundle(bundle: &TraceBundle) -> EvalCaseActual {
    EvalCaseActual {
        answer: trace_last_event_payload_text(bundle, TraceEventKind::FinalAnswer, "answer"),
        citations: trace_bundle_citations(bundle),
        intent: None,
        tool_code: trace_event_payload_text(bundle, TraceEventKind::ToolCall, "toolCode"),
        cost_cents: 0,
        latency_ms: 0,
    }
}

fn trace_event_payload_text(
    bundle: &TraceBundle,
    kind: TraceEventKind,
    key: &str,
) -> Option<String> {
    bundle
        .events
        .iter()
        .find(|event| event.kind == kind)
        .and_then(|event| trace_value_text(event.payload.get(key)))
}

fn trace_last_event_payload_text(
    bundle: &TraceBundle,
    kind: TraceEventKind,
    key: &str,
) -> Option<String> {
    bundle
        .events
        .iter()
        .rev()
        .find(|event| event.kind == kind)
        .and_then(|event| trace_value_text(event.payload.get(key)))
}

fn trace_event_count(bundle: &TraceBundle, kind: TraceEventKind) -> usize {
    bundle
        .events
        .iter()
        .filter(|event| event.kind == kind)
        .count()
}

#[derive(Debug, Default)]
struct TraceCompactionSummary {
    model_count: usize,
    fallback_count: usize,
    remote_count: usize,
    status: Option<String>,
    implementation: Option<String>,
}

fn trace_compaction_summary(bundle: &TraceBundle) -> TraceCompactionSummary {
    let mut summary = TraceCompactionSummary::default();
    for event in bundle
        .events
        .iter()
        .filter(|event| event.kind == TraceEventKind::ContextCompaction)
    {
        match trace_value_text(event.payload.get("compactionStrategy")).as_deref() {
            Some("model") => summary.model_count += 1,
            Some("deterministic_fallback") => summary.fallback_count += 1,
            _ => {}
        }
        if let Some(status) = trace_value_text(event.payload.get("compactionStatus")) {
            summary.status = Some(status);
        }
        if let Some(implementation) =
            trace_value_text(event.payload.get("compactionImplementation"))
        {
            summary.implementation = Some(implementation.clone());
            if implementation == "responses_compaction_v2" {
                summary.remote_count += 1;
            }
        } else if event.payload.get("remoteCompaction").is_some() {
            summary.remote_count += 1;
        }
    }
    summary
}

fn trace_first_cancellation_reason(bundle: &TraceBundle) -> Option<String> {
    bundle
        .events
        .iter()
        .find(|event| event.kind == TraceEventKind::Cancellation)
        .and_then(|event| event.payload.get("cancelReason"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[derive(Debug, Default)]
struct TraceGuardianReviewSummary {
    outcome: Option<String>,
    source: Option<String>,
    requires_human_approval: Option<bool>,
    review_status: Option<String>,
    failure_reason: Option<String>,
    model_route_id: Option<String>,
    auto_approved: Option<bool>,
}

fn trace_guardian_review_summary(bundle: &TraceBundle) -> TraceGuardianReviewSummary {
    let Some(event) = bundle.events.iter().find(|event| {
        matches!(
            event.kind,
            TraceEventKind::ApprovalRequested | TraceEventKind::ActionSelected
        ) && event.payload.get("guardianReview").is_some()
    }) else {
        return TraceGuardianReviewSummary::default();
    };
    let Some(review) = event.payload.get("guardianReview") else {
        return TraceGuardianReviewSummary::default();
    };

    TraceGuardianReviewSummary {
        outcome: trace_value_text(review.get("outcome")),
        source: trace_value_text(review.get("source")),
        requires_human_approval: review
            .get("requiresHumanApproval")
            .or_else(|| review.get("requires_human_approval"))
            .and_then(Value::as_bool),
        review_status: trace_value_text(
            review
                .get("reviewStatus")
                .or_else(|| review.get("review_status")),
        ),
        failure_reason: trace_value_text(
            review
                .get("failureReason")
                .or_else(|| review.get("failure_reason")),
        ),
        model_route_id: trace_value_text(
            review
                .get("modelRouteId")
                .or_else(|| review.get("model_route_id")),
        ),
        auto_approved: event
            .payload
            .get("guardianAutoApproved")
            .or_else(|| event.payload.get("guardian_auto_approved"))
            .and_then(Value::as_bool)
            .or_else(|| {
                trace_value_text(event.payload.get("approvalMode"))
                    .map(|mode| mode == "guardian_auto_approved")
            }),
    }
}

#[derive(Debug, Default)]
struct TraceRuntimeSupervisorSummary {
    task_kind: Option<String>,
    cancel_signal_sent: Option<bool>,
    active_before_cancel: Option<bool>,
}

fn trace_runtime_supervisor_summary(bundle: &TraceBundle) -> TraceRuntimeSupervisorSummary {
    let Some(payload) = bundle
        .events
        .iter()
        .find(|event| event.kind == TraceEventKind::Cancellation)
        .map(|event| &event.payload)
    else {
        return TraceRuntimeSupervisorSummary::default();
    };
    let supervisor = payload.get("runtimeSupervisor");

    TraceRuntimeSupervisorSummary {
        task_kind: supervisor
            .and_then(|value| value.get("taskKind"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        cancel_signal_sent: payload.get("runtimeSignalSent").and_then(Value::as_bool),
        active_before_cancel: supervisor
            .and_then(|value| value.get("activeBeforeCancel"))
            .and_then(Value::as_bool),
    }
}

#[derive(Debug, Default)]
struct TraceToolIoTaskSummary {
    count: usize,
    parallel_count: usize,
    serial_count: usize,
    cancelled_count: usize,
    timeout_count: usize,
    max_duration_ms: i64,
    supervisors: BTreeSet<String>,
}

fn trace_tool_io_task_summary(bundle: &TraceBundle) -> TraceToolIoTaskSummary {
    let mut summary = TraceToolIoTaskSummary::default();
    for event in bundle
        .events
        .iter()
        .filter(|event| event.kind == TraceEventKind::Observation)
    {
        let Some(task) = trace_tool_io_task_payload(&event.payload) else {
            continue;
        };
        summary.count += 1;
        match trace_value_text(task.get("executionMode")).as_deref() {
            Some("parallel") => summary.parallel_count += 1,
            Some("serial") => summary.serial_count += 1,
            _ => {}
        }
        if trace_value_text(task.get("terminalStatus")).as_deref() == Some("cancelled") {
            summary.cancelled_count += 1;
        }
        if trace_value_text(task.get("cancelReason")).as_deref() == Some("tool_io_timeout") {
            summary.timeout_count += 1;
        }
        if let Some(duration_ms) = trace_value_i64(task.get("durationMs")) {
            summary.max_duration_ms = summary.max_duration_ms.max(duration_ms);
        }
        if let Some(supervisor) = trace_value_text(task.get("supervisor")) {
            summary.supervisors.insert(supervisor);
        }
    }
    summary
}

fn trace_tool_io_task_payload(payload: &Value) -> Option<&Value> {
    payload.get("toolIoTask").or_else(|| {
        payload
            .get("output")
            .and_then(|output| output.get("toolIoTask"))
    })
}

#[derive(Debug, Default)]
struct TraceInferenceSummary {
    count: usize,
    route_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    latency_ms: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    cost_cents: Option<f64>,
    error_count: usize,
    retry_count: usize,
    retryable_error_count: usize,
    provider_attempt_count: usize,
    delta_count: usize,
    delta_text_length: usize,
    streaming_tool_call_count: usize,
    streaming_tool_codes: BTreeSet<String>,
    provider_native_cancel_count: usize,
    provider_native_cancel_attempted: Option<bool>,
    provider_native_cancel_supported: Option<bool>,
    provider_native_cancel_provider: Option<String>,
    provider_native_cancel_status: Option<String>,
    provider_native_cancel_http_status: Option<i64>,
    fallback_count: usize,
    circuit_open_count: usize,
    fallback_route_id: Option<String>,
    error_kind: Option<String>,
    http_status: Option<i64>,
}

fn trace_inference_summary(bundle: &TraceBundle) -> TraceInferenceSummary {
    let mut summary = TraceInferenceSummary::default();
    for event in bundle
        .events
        .iter()
        .filter(|event| event.kind == TraceEventKind::Inference)
    {
        let payload = trace_inference_payload(&event.payload);
        summary.count += 1;
        if summary.route_id.is_none() {
            summary.route_id = trace_value_text(payload.get("routeId"));
        }
        if summary.provider.is_none() {
            summary.provider = trace_value_text(payload.get("provider"));
        }
        if summary.model.is_none() {
            summary.model = trace_value_text(payload.get("model"));
        }
        let inference_type = trace_value_text(payload.get("type"));
        if inference_type.as_deref() == Some("model_delta") {
            summary.delta_count += 1;
            summary.delta_text_length += trace_value_raw_text(payload.get("content"))
                .map(|content| content.chars().count())
                .unwrap_or_default();
        }
        if inference_type.as_deref() == Some("model_stream_tool_call") {
            let tool_calls = payload.get("toolCalls").and_then(Value::as_array);
            let tool_call_count = trace_value_i64(payload.get("toolCallCount"))
                .filter(|count| *count > 0)
                .map(|count| count as usize)
                .or_else(|| tool_calls.map(Vec::len))
                .unwrap_or(1);
            summary.streaming_tool_call_count += tool_call_count;
            if let Some(tool_calls) = tool_calls {
                for tool_call in tool_calls {
                    if let Some(tool_code) = trace_value_text(tool_call.get("toolCode")) {
                        summary.streaming_tool_codes.insert(tool_code);
                    }
                }
            }
        }
        if matches!(
            inference_type.as_deref(),
            Some("provider_native_cancel") | Some("provider_native_cancel_error")
        ) {
            summary.provider_native_cancel_count += 1;
            if summary.provider_native_cancel_attempted.is_none() {
                summary.provider_native_cancel_attempted =
                    payload.get("attempted").and_then(Value::as_bool);
            }
            if summary.provider_native_cancel_supported.is_none() {
                summary.provider_native_cancel_supported =
                    payload.get("supported").and_then(Value::as_bool);
            }
            if summary.provider_native_cancel_provider.is_none() {
                summary.provider_native_cancel_provider = trace_value_text(payload.get("provider"));
            }
            if summary.provider_native_cancel_status.is_none() {
                summary.provider_native_cancel_status = trace_value_text(payload.get("status"));
            }
            if summary.provider_native_cancel_http_status.is_none() {
                summary.provider_native_cancel_http_status =
                    trace_value_i64(payload.get("httpStatus"));
            }
        }
        if inference_type.as_deref() == Some("model_inference_error") {
            summary.error_count += 1;
            if payload
                .get("retryable")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                summary.retryable_error_count += 1;
            }
            if payload
                .get("willRetry")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                summary.retry_count += 1;
            }
            if summary.error_kind.is_none() {
                summary.error_kind = trace_value_text(payload.get("errorKind"));
            }
            if summary.http_status.is_none() {
                summary.http_status = trace_value_i64(payload.get("httpStatus"));
            }
        }
        if let Some(attempts) = payload.get("providerAttempts").and_then(Value::as_array) {
            summary.provider_attempt_count += attempts.len();
            for attempt in attempts {
                let attempt_kind = trace_value_text(attempt.get("attemptKind"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let status = trace_value_text(attempt.get("status"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let error_kind = trace_value_text(attempt.get("errorKind"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if attempt_kind == "fallback" && status == "succeeded" {
                    summary.fallback_count += 1;
                    if summary.fallback_route_id.is_none() {
                        summary.fallback_route_id = trace_value_text(attempt.get("routeId"));
                    }
                }
                if error_kind == "circuit_open" {
                    summary.circuit_open_count += 1;
                }
            }
        }
        summary.latency_ms += trace_value_i64(payload.get("latencyMs")).unwrap_or_default();
        if let Some(usage) = payload.get("usage") {
            summary.prompt_tokens += trace_value_i64(
                usage
                    .get("promptTokens")
                    .or_else(|| usage.get("prompt_tokens")),
            )
            .unwrap_or_default();
            summary.completion_tokens += trace_value_i64(
                usage
                    .get("completionTokens")
                    .or_else(|| usage.get("completion_tokens")),
            )
            .unwrap_or_default();
            summary.total_tokens += trace_value_i64(
                usage
                    .get("totalTokens")
                    .or_else(|| usage.get("total_tokens")),
            )
            .unwrap_or_default();
        }
        if let Some(cost_cents) = trace_value_f64(payload.get("costCents")) {
            summary.cost_cents = Some(summary.cost_cents.unwrap_or_default() + cost_cents);
        }
    }
    summary
}

fn trace_inference_payload(payload: &Value) -> &Value {
    payload.get("item").unwrap_or(payload)
}

fn trace_value_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| {
                number
                    .as_u64()
                    .map(|value| value.min(i64::MAX as u64) as i64)
            })
            .or_else(|| number.as_f64().map(|value| value.round() as i64)),
        _ => None,
    }
}

fn trace_value_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64().filter(|value| value.is_finite()),
        _ => None,
    }
}

fn trace_value_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        }
        Value::Null => None,
        value => Some(value.to_string()),
    }
}

fn trace_value_raw_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.to_owned()),
        Value::Null => None,
        value => Some(value.to_string()),
    }
}

fn trace_answer_snippet(answer: &str, max_chars: usize) -> String {
    answer.trim().chars().take(max_chars).collect()
}

fn trace_bundle_citations(bundle: &TraceBundle) -> Vec<String> {
    let mut citations = Vec::new();
    for event in &bundle.events {
        collect_citations_from_value(&event.payload, &mut citations);
    }
    citations.sort();
    citations.dedup();
    citations
}

fn collect_citations_from_value(value: &Value, citations: &mut Vec<String>) {
    let Some(values) = value.get("citations").and_then(Value::as_array) else {
        return;
    };
    citations.extend(values.iter().filter_map(|value| {
        match value {
            Value::String(citation) => Some(citation.trim().to_owned()),
            Value::Object(object) => object
                .get("chunkId")
                .or_else(|| object.get("chunk_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .map(ToOwned::to_owned),
            _ => None,
        }
    }));
    citations.retain(|citation| !citation.is_empty());
}
