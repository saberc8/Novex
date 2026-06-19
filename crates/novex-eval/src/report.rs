use std::collections::BTreeMap;

use crate::case::EvalMetricKind;
use crate::score::EvalCaseScore;
use crate::text::round_score;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionReport {
    pub total_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    pub average_score: f64,
    pub metric_breakdown: BTreeMap<EvalMetricKind, f64>,
    pub total_cost_cents: u32,
    pub total_latency_ms: u32,
}

pub fn build_regression_report(scores: &[EvalCaseScore]) -> RegressionReport {
    let total_cases = scores.len();
    let passed_cases = scores.iter().filter(|score| score.passed).count();
    let failed_cases = total_cases.saturating_sub(passed_cases);
    let average_score = if total_cases == 0 {
        0.0
    } else {
        round_score(scores.iter().map(|score| score.score).sum::<f64>() / total_cases as f64)
    };
    let mut metric_totals = BTreeMap::<EvalMetricKind, (f64, usize)>::new();
    for score in scores {
        let entry = metric_totals.entry(score.metric).or_insert((0.0, 0));
        entry.0 += score.score;
        entry.1 += 1;
    }
    let metric_breakdown = metric_totals
        .into_iter()
        .map(|(metric, (total, count))| (metric, round_score(total / count as f64)))
        .collect();

    RegressionReport {
        total_cases,
        passed_cases,
        failed_cases,
        average_score,
        metric_breakdown,
        total_cost_cents: scores.iter().map(|score| score.cost_cents).sum(),
        total_latency_ms: scores.iter().map(|score| score.latency_ms).sum(),
    }
}
