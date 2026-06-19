use novex_eval::*;
use novex_trace::{TraceBundle, TraceEvent, TraceEventKind};
use serde_json::json;

#[test]
fn trace_eval_candidate_extracts_tool_and_final_answer() {
    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle_with_tool_and_final());

    assert_eq!(candidate.target_kind, EvalTargetKind::ReAct);
    assert_eq!(candidate.expected.tool_code.as_deref(), Some("rag.search"));
    assert!(candidate.prompt.contains("customer data"));
    assert!(candidate
        .expected
        .answer_contains
        .iter()
        .any(|snippet| snippet.contains("approved systems")));
}

#[test]
fn trace_eval_candidate_tags_runtime_spans() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::retrieval(1, json!({"hitCount":2})))
        .with_event(TraceEvent::context_compaction(
            2,
            json!({"compactedItemCount":4}),
        ))
        .with_event(TraceEvent::cancellation(
            3,
            json!({"cancelReason":"external_cancel"}),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["retrievalCount"], 1);
    assert_eq!(candidate.tags["compactionCount"], 1);
    assert_eq!(candidate.tags["cancelled"], true);
    assert_eq!(candidate.tags["cancelReason"], "external_cancel");
}

#[test]
fn runtime_supervisor_trace_eval_candidate_tags_runtime_cancellation() {
    let bundle = TraceBundle::new("agent-supervisor")
        .with_event(TraceEvent::user_message(1, "stop"))
        .with_event(TraceEvent::cancellation(
            2,
            json!({
                "cancelReason": "external_cancel",
                "runtimeSignalSent": true,
                "runtimeSupervisor": {
                    "activeBeforeCancel": true,
                    "taskKind": "model_loop",
                    "status": "cancelling"
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["runtimeSupervisorTaskKind"], "model_loop");
    assert_eq!(candidate.tags["runtimeSupervisorCancelSignalSent"], true);
    assert_eq!(candidate.tags["runtimeSupervisorActiveBeforeCancel"], true);
}

#[test]
fn tool_io_observability_trace_eval_candidate_tags_task_metrics() {
    let bundle = TraceBundle::new("agent-tool-io")
        .with_event(TraceEvent {
            sequence_no: 1,
            kind: TraceEventKind::Observation,
            payload: json!({
                "callId": "call-1",
                "output": {"status": "succeeded"},
                "toolIoTask": {
                    "executionMode": "parallel",
                    "taskRuntime": "tokio_task",
                    "supervisor": "agent_tool_io_task_supervisor",
                    "durationMs": 9,
                    "terminalStatus": "succeeded"
                }
            }),
        })
        .with_event(TraceEvent {
            sequence_no: 2,
            kind: TraceEventKind::Observation,
            payload: json!({
                "callId": "call-2",
                "output": {"status": "cancelled"},
                "toolIoTask": {
                    "executionMode": "serial",
                    "taskRuntime": "inline",
                    "supervisor": "agent_tool_io_task_supervisor",
                    "durationMs": 15,
                    "terminalStatus": "cancelled",
                    "cancelReason": "tool_io_timeout"
                }
            }),
        });

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["toolIoTaskCount"], 2);
    assert_eq!(candidate.tags["parallelToolIoTaskCount"], 1);
    assert_eq!(candidate.tags["serialToolIoTaskCount"], 1);
    assert_eq!(candidate.tags["cancelledToolIoTaskCount"], 1);
    assert_eq!(candidate.tags["timeoutToolIoTaskCount"], 1);
    assert_eq!(candidate.tags["toolIoTaskMaxDurationMs"], 15);
    assert_eq!(
        candidate.tags["toolIoTaskSupervisors"],
        json!(["agent_tool_io_task_supervisor"])
    );
}

#[test]
fn guardian_review_trace_eval_candidate_tags_approval_review() {
    let bundle = TraceBundle::new("trace-guardian")
        .with_event(TraceEvent::user_message(1, "write an issue"))
        .with_event(TraceEvent {
            sequence_no: 2,
            kind: TraceEventKind::ApprovalRequested,
            payload: json!({
                "toolCode": "github.issue.write",
                "guardianReview": {
                    "outcome": "needs_human",
                    "source": "policy",
                    "requiresHumanApproval": true
                }
            }),
        });

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["guardianReviewOutcome"], "needs_human");
    assert_eq!(candidate.tags["guardianReviewSource"], "policy");
    assert_eq!(candidate.tags["guardianReviewRequiresHumanApproval"], true);
}

#[test]
fn guardian_auto_approval_trace_eval_candidate_tags_action_review() {
    let bundle = TraceBundle::new("trace-guardian-auto-approved")
        .with_event(TraceEvent::user_message(1, "write an issue"))
        .with_event(TraceEvent::action_selected(
            2,
            json!({
                "toolCode": "github.issue.write",
                "approvalMode": "guardian_auto_approved",
                "guardianAutoApproved": true,
                "guardianReview": {
                    "outcome": "approved",
                    "source": "guardian",
                    "requiresHumanApproval": false,
                    "reviewStatus": "reviewed",
                    "modelRouteId": "runtime.llm.guardian"
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["guardianAutoApproved"], true);
    assert_eq!(candidate.tags["guardianReviewOutcome"], "approved");
    assert_eq!(candidate.tags["guardianReviewSource"], "guardian");
    assert_eq!(candidate.tags["guardianReviewStatus"], "reviewed");
    assert_eq!(
        candidate.tags["guardianReviewModelRouteId"],
        "runtime.llm.guardian"
    );
}

#[test]
fn guardian_model_review_trace_eval_candidate_tags_reviewer_metadata() {
    let bundle = TraceBundle::new("trace-guardian-model")
        .with_event(TraceEvent::user_message(1, "write an issue"))
        .with_event(TraceEvent {
            sequence_no: 2,
            kind: TraceEventKind::ApprovalRequested,
            payload: json!({
                "toolCode": "github.issue.write",
                "guardianReview": {
                    "outcome": "needs_human",
                    "source": "guardian",
                    "requiresHumanApproval": true,
                    "reviewStatus": "failed_closed",
                    "failureReason": "timeout",
                    "modelRouteId": "runtime.llm.guardian",
                    "modelProvider": "deep-seek",
                    "modelName": "deepseek-v4-flash"
                }
            }),
        });

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["guardianReviewStatus"], "failed_closed");
    assert_eq!(candidate.tags["guardianReviewFailureReason"], "timeout");
    assert_eq!(
        candidate.tags["guardianReviewModelRouteId"],
        "runtime.llm.guardian"
    );
}

#[test]
fn trace_eval_candidate_tags_model_compaction_strategy() {
    let bundle = TraceBundle::new("trace-compact")
        .with_event(TraceEvent::user_message(1, "answer from a long notebook"))
        .with_event(TraceEvent::context_compaction(
            2,
            json!({
                "item": {"type":"context_compaction","summary":"model summary"},
                "compactionStrategy": "model",
                "compactionStatus": "succeeded"
            }),
        ))
        .with_event(TraceEvent::final_answer(3, "done"));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["compactionCount"], 1);
    assert_eq!(candidate.tags["modelCompactionCount"], 1);
    assert_eq!(candidate.tags["compactionFallbackCount"], 0);
    assert_eq!(candidate.tags["compactionStatus"], "succeeded");
}

#[test]
fn remote_compaction_trace_eval_candidate_tags_endpoint_contract() {
    let bundle =
        TraceBundle::new("trace-remote-compact").with_event(TraceEvent::context_compaction(
            1,
            json!({
                "compactionStrategy": "model",
                "compactionStatus": "succeeded",
                "compactionImplementation": "responses_compaction_v2",
                "remoteCompaction": {
                    "implementation": "responses_compaction_v2",
                    "trigger": "auto",
                    "reason": "observation_threshold"
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["remoteCompactionCount"], 1);
    assert_eq!(
        candidate.tags["compactionImplementation"],
        "responses_compaction_v2"
    );
}

#[test]
fn trace_eval_candidate_tags_inference_spans() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::inference(
            1,
            json!({
                "item": {
                    "type": "model_inference",
                    "routeId": "runtime.llm.code_agent",
                    "provider": "deep-seek",
                    "model": "deepseek-v4-flash",
                    "latencyMs": 42,
                    "usage": {
                        "promptTokens": 11,
                        "completionTokens": 7,
                        "totalTokens": 18
                    },
                    "costCents": 0.65
                }
            }),
        ))
        .with_event(TraceEvent::inference(
            2,
            json!({
                "item": {
                    "type": "model_inference",
                    "latencyMs": 8,
                    "usage": {
                        "promptTokens": 3,
                        "completionTokens": 2,
                        "totalTokens": 5
                    }
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["inferenceCount"], 2);
    assert_eq!(candidate.tags["modelProvider"], "deep-seek");
    assert_eq!(candidate.tags["modelRouteId"], "runtime.llm.code_agent");
    assert_eq!(candidate.tags["modelName"], "deepseek-v4-flash");
    assert_eq!(candidate.tags["latencyMs"], 50);
    assert_eq!(candidate.tags["promptTokens"], 14);
    assert_eq!(candidate.tags["completionTokens"], 9);
    assert_eq!(candidate.tags["totalTokens"], 23);
    assert_eq!(candidate.tags["costCents"], 0.65);
}

#[test]
fn trace_eval_candidate_tags_model_delta_streaming() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::inference(
            1,
            json!({
                "item": {
                    "type": "model_delta",
                    "source": "provider_stream",
                    "routeId": "runtime.llm.code_agent",
                    "provider": "openai-compatible",
                    "model": "gpt-compatible",
                    "deltaIndex": 0,
                    "content": "Hello",
                    "providerEvent": "chat.completion.chunk"
                }
            }),
        ))
        .with_event(TraceEvent::inference(
            2,
            json!({
                "item": {
                    "type": "model_delta",
                    "source": "provider_stream",
                    "routeId": "runtime.llm.code_agent",
                    "provider": "openai-compatible",
                    "model": "gpt-compatible",
                    "deltaIndex": 1,
                    "content": " world",
                    "providerEvent": "chat.completion.chunk"
                }
            }),
        ))
        .with_event(TraceEvent::inference(
            3,
            json!({
                "item": {
                    "type": "model_inference",
                    "routeId": "runtime.llm.code_agent",
                    "provider": "openai-compatible",
                    "model": "gpt-compatible",
                    "streaming": true,
                    "deltaChunkCount": 2,
                    "deltaTextLength": 11,
                    "latencyMs": 42,
                    "usage": {
                        "promptTokens": 11,
                        "completionTokens": 7,
                        "totalTokens": 18
                    }
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["inferenceCount"], 3);
    assert_eq!(candidate.tags["modelDeltaCount"], 2);
    assert_eq!(candidate.tags["modelDeltaTextLength"], 11);
    assert_eq!(candidate.tags["streamingModelOutput"], true);
    assert_eq!(candidate.tags["modelProvider"], "openai-compatible");
    assert_eq!(candidate.tags["modelRouteId"], "runtime.llm.code_agent");
}

#[test]
fn trace_eval_candidate_tags_streaming_tool_call_detection() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "item": {
                "type": "model_stream_tool_call",
                "source": "provider_stream",
                "routeId": "runtime.llm.code_agent",
                "provider": "openai-compatible",
                "model": "gpt-compatible",
                "deltaIndex": 1,
                "toolCallCount": 2,
                "toolCalls": [
                    {
                        "callId": "call-1",
                        "toolCode": "rag.search",
                        "arguments": {"query": "policy"}
                    },
                    {
                        "callId": "call-2",
                        "toolCode": "github.repo.read",
                        "arguments": {"repository": "org/repo", "path": "README.md"}
                    }
                ]
            }
        }),
    ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["streamingToolCallDetected"], true);
    assert_eq!(candidate.tags["streamingToolCallCount"], 2);
    assert_eq!(
        candidate.tags["streamingToolCodes"],
        json!(["github.repo.read", "rag.search"])
    );
}

#[test]
fn provider_native_cancel_trace_eval_candidate_tags_cancel_attempt() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "item": {
                "type": "provider_native_cancel",
                "providerCallLeaseId": 4242,
                "status": "cancelled",
                "attempted": true,
                "supported": true,
                "provider": "openai-compatible",
                "providerResponseId": "resp_stream_1",
                "httpStatus": 200,
                "message": "native_cancel_sent"
            }
        }),
    ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["providerNativeCancelCount"], 1);
    assert_eq!(candidate.tags["providerNativeCancelAttempted"], true);
    assert_eq!(candidate.tags["providerNativeCancelSupported"], true);
    assert_eq!(
        candidate.tags["providerNativeCancelProvider"],
        "openai-compatible"
    );
    assert_eq!(candidate.tags["providerNativeCancelStatus"], "cancelled");
    assert_eq!(candidate.tags["providerNativeCancelHttpStatus"], 200);
}

#[test]
fn trace_eval_candidate_tags_provider_error_spans() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "item": {
                "type": "model_inference_error",
                "routeId": "runtime.llm.code_agent",
                "provider": "deep-seek",
                "errorKind": "provider_http",
                "httpStatus": 502,
                "retryable": true,
                "latencyMs": 12
            }
        }),
    ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["inferenceCount"], 1);
    assert_eq!(candidate.tags["inferenceErrorCount"], 1);
    assert_eq!(candidate.tags["retryableInferenceErrorCount"], 1);
    assert_eq!(candidate.tags["modelErrorKind"], "provider_http");
    assert_eq!(candidate.tags["modelHttpStatus"], 502);
    assert_eq!(candidate.tags["latencyMs"], 12);
}

#[test]
fn trace_eval_candidate_tags_provider_retry_spans() {
    let bundle = TraceBundle::new("agent-1")
        .with_event(TraceEvent::inference(
            1,
            json!({
                "item": {
                    "type": "model_inference_error",
                    "errorKind": "provider_http",
                    "httpStatus": 502,
                    "retryable": true,
                    "willRetry": true,
                    "latencyMs": 12
                }
            }),
        ))
        .with_event(TraceEvent::inference(
            2,
            json!({
                "item": {
                    "type": "model_inference",
                    "latencyMs": 20
                }
            }),
        ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["inferenceErrorCount"], 1);
    assert_eq!(candidate.tags["modelRetryCount"], 1);
    assert_eq!(candidate.tags["latencyMs"], 32);
}

#[test]
fn trace_eval_candidate_tags_provider_fallback_attempts() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "item": {
                "type": "model_inference",
                "routeId": "runtime.llm.backup",
                "providerAttempts": [
                    {
                        "attemptKind": "primary",
                        "routeId": "runtime.llm",
                        "status": "failed"
                    },
                    {
                        "attemptKind": "fallback",
                        "routeId": "runtime.llm.backup",
                        "status": "succeeded"
                    }
                ]
            }
        }),
    ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["modelProviderAttemptCount"], 2);
    assert_eq!(candidate.tags["modelFallbackCount"], 1);
    assert_eq!(candidate.tags["modelFallbackRouteId"], "runtime.llm.backup");
}

#[test]
fn trace_eval_candidate_tags_circuit_breaker_attempts() {
    let bundle = TraceBundle::new("agent-1").with_event(TraceEvent::inference(
        1,
        json!({
            "item": {
                "type": "model_inference",
                "providerAttempts": [
                    {
                        "attemptKind": "primary",
                        "routeId": "runtime.llm",
                        "status": "skipped",
                        "errorKind": "circuit_open"
                    },
                    {
                        "attemptKind": "fallback",
                        "routeId": "runtime.llm.backup",
                        "status": "succeeded"
                    }
                ]
            }
        }),
    ));

    let candidate = EvalCaseCandidate::from_trace_bundle(&bundle);

    assert_eq!(candidate.tags["modelCircuitOpenCount"], 1);
    assert_eq!(candidate.tags["modelFallbackCount"], 1);
}

#[test]
fn trace_eval_actual_extracts_tool_and_final_answer() {
    let actual = actual_from_trace_bundle(&bundle_with_tool_and_final());

    assert_eq!(actual.tool_code.as_deref(), Some("rag.search"));
    assert_eq!(
        actual.answer.as_deref(),
        Some("Customer data must stay in approved systems.")
    );
}

fn bundle_with_tool_and_final() -> TraceBundle {
    TraceBundle::new("agent-1")
        .with_event(TraceEvent::user_message(
            1,
            "How should we handle customer data?",
        ))
        .with_event(TraceEvent::tool_call(2, "call-1", "rag.search"))
        .with_event(TraceEvent::final_answer(
            3,
            "Customer data must stay in approved systems.",
        ))
}
