use std::collections::BTreeSet;

use crate::router::RoutedToolCall;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionLock {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConcurrencyPolicy {
    pub lock: ToolExecutionLock,
    pub supports_parallel_calls: bool,
    pub waits_for_runtime_cancellation: bool,
    pub exclusive_group: Option<String>,
}

impl ToolConcurrencyPolicy {
    pub fn shared() -> Self {
        Self {
            lock: ToolExecutionLock::Shared,
            supports_parallel_calls: true,
            waits_for_runtime_cancellation: false,
            exclusive_group: None,
        }
    }

    pub fn exclusive(group: impl Into<String>) -> Self {
        let group = group.into().trim().to_owned();
        Self {
            lock: ToolExecutionLock::Exclusive,
            supports_parallel_calls: false,
            waits_for_runtime_cancellation: false,
            exclusive_group: (!group.is_empty()).then_some(group),
        }
    }

    pub fn exclusive_waits_for_runtime_cancellation(group: impl Into<String>) -> Self {
        Self {
            waits_for_runtime_cancellation: true,
            ..Self::exclusive(group)
        }
    }
}

impl Default for ToolConcurrencyPolicy {
    fn default() -> Self {
        Self {
            lock: ToolExecutionLock::Exclusive,
            supports_parallel_calls: false,
            waits_for_runtime_cancellation: false,
            exclusive_group: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolBatchExecutionMode {
    Parallel,
    Serial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolBatchPlan {
    pub mode: ToolBatchExecutionMode,
    pub calls: Vec<RoutedToolCall>,
    pub serial_reason: Option<String>,
}

impl ToolBatchPlan {
    pub fn from_routed_calls(calls: Vec<RoutedToolCall>) -> Self {
        let mut exclusive_groups = BTreeSet::new();
        for call in &calls {
            let policy = &call.tool.concurrency;
            if let Some(group) = policy.exclusive_group.as_deref() {
                if !exclusive_groups.insert(group.to_owned()) {
                    return Self {
                        mode: ToolBatchExecutionMode::Serial,
                        serial_reason: Some(format!("exclusive_group:{group}")),
                        calls,
                    };
                }
            }
        }

        for call in &calls {
            let policy = &call.tool.concurrency;
            if policy.lock == ToolExecutionLock::Exclusive || !policy.supports_parallel_calls {
                return Self {
                    mode: ToolBatchExecutionMode::Serial,
                    serial_reason: Some(format!("exclusive_tool:{}", call.tool.code)),
                    calls,
                };
            }
        }

        Self {
            mode: ToolBatchExecutionMode::Parallel,
            calls,
            serial_reason: None,
        }
    }
}
