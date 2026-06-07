use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    application::system::format_datetime,
    infrastructure::persistence::ai_knowledge_repository::{
        AiKnowledgeRepository, TrainingLearningActivityRecord, TrainingLearningSummaryRecord,
        TrainingWeakPointRecord,
    },
    shared::error::AppError,
};

const DEFAULT_SCOPE: &str = "self";
const TENANT_SCOPE: &str = "tenant";
const LEARNING_ACTIVITY_LIMIT: i64 = 8;
const WEAK_POINT_LIMIT: i64 = 5;

#[derive(Debug, Clone)]
pub struct TrainingService {
    repo: AiKnowledgeRepository,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingLearningQuery {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingLearningRecordsResp {
    pub scope: String,
    pub summary: TrainingLearningSummaryResp,
    pub tasks: Vec<TrainingLearningTaskResp>,
    pub records: Vec<TrainingLearningRecordResp>,
    pub weak_points: Vec<TrainingWeakPointResp>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingLearningSummaryResp {
    pub completion_rate: i32,
    pub pending_task_count: i32,
    pub quiz_average_score: i32,
    pub weak_point_count: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingLearningTaskResp {
    pub title: String,
    pub source: String,
    pub due: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingLearningRecordResp {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub status: String,
    pub score: Option<f64>,
    pub learner_id: i64,
    pub learner_name: String,
    pub create_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingWeakPointResp {
    pub topic: String,
    pub evidence: String,
    pub count: i64,
    pub last_seen_at: String,
}

impl TrainingService {
    pub fn new(db: PgPool) -> Self {
        Self {
            repo: AiKnowledgeRepository::new(db),
        }
    }

    pub async fn list_learning_records_for_tenant(
        &self,
        tenant_id: i64,
        current_user_id: i64,
        query: TrainingLearningQuery,
    ) -> Result<TrainingLearningRecordsResp, AppError> {
        let (scope, target_user_id) = normalize_learning_query(current_user_id, query)?;
        let summary = self
            .repo
            .training_learning_summary(tenant_id, target_user_id)
            .await?;
        let records = self
            .repo
            .list_training_learning_activities(tenant_id, target_user_id, LEARNING_ACTIVITY_LIMIT)
            .await?
            .into_iter()
            .map(TrainingLearningRecordResp::from)
            .collect::<Vec<_>>();
        let weak_points = self
            .repo
            .list_training_weak_points(tenant_id, target_user_id, WEAK_POINT_LIMIT)
            .await?
            .into_iter()
            .map(TrainingWeakPointResp::from)
            .collect::<Vec<_>>();
        let tasks = training_learning_tasks(&summary);
        let response_summary =
            training_learning_summary_response(&summary, &tasks, weak_points.len());

        Ok(TrainingLearningRecordsResp {
            scope,
            summary: response_summary,
            tasks,
            records,
            weak_points,
        })
    }
}

impl From<TrainingLearningActivityRecord> for TrainingLearningRecordResp {
    fn from(record: TrainingLearningActivityRecord) -> Self {
        Self {
            id: record.id,
            kind: record.kind,
            title: record.title,
            detail: record.detail,
            status: record.status,
            score: record.score,
            learner_id: record.learner_id,
            learner_name: record.learner_name,
            create_time: format_datetime(record.create_time),
        }
    }
}

impl From<TrainingWeakPointRecord> for TrainingWeakPointResp {
    fn from(record: TrainingWeakPointRecord) -> Self {
        Self {
            topic: record.topic,
            evidence: record.evidence,
            count: record.count,
            last_seen_at: format_datetime(record.last_seen_at),
        }
    }
}

fn normalize_learning_query(
    current_user_id: i64,
    query: TrainingLearningQuery,
) -> Result<(String, Option<i64>), AppError> {
    let scope = query
        .scope
        .unwrap_or_else(|| DEFAULT_SCOPE.to_owned())
        .trim()
        .to_ascii_lowercase();
    match scope.as_str() {
        "" | DEFAULT_SCOPE => Ok((
            DEFAULT_SCOPE.to_owned(),
            Some(query.user_id.unwrap_or(current_user_id)),
        )),
        TENANT_SCOPE => Ok((TENANT_SCOPE.to_owned(), None)),
        _ => Err(AppError::bad_request("学习记录范围不合法")),
    }
}

fn training_learning_tasks(
    summary: &TrainingLearningSummaryRecord,
) -> Vec<TrainingLearningTaskResp> {
    let has_rag_activity = summary.rag_trace_count > 0;
    let has_quiz_activity =
        summary.latest_eval_total_cases.unwrap_or_default() > 0 || summary.quiz_wrong_count > 0;
    let has_weak_points = summary.weak_signal_count > 0;

    vec![
        TrainingLearningTaskResp {
            title: "完成信息安全入职培训".to_owned(),
            source: "入职制度知识库".to_owned(),
            due: "今日 18:00".to_owned(),
            status: if has_rag_activity {
                "进行中".to_owned()
            } else {
                "未开始".to_owned()
            },
        },
        TrainingLearningTaskResp {
            title: "完成 5 题培训测验".to_owned(),
            source: "培训出题 Skill".to_owned(),
            due: "今日 20:00".to_owned(),
            status: if has_quiz_activity {
                "已完成".to_owned()
            } else {
                "未开始".to_owned()
            },
        },
        TrainingLearningTaskResp {
            title: "复盘本周错题".to_owned(),
            source: "测验记录".to_owned(),
            due: "周五前".to_owned(),
            status: if has_weak_points {
                "待复习".to_owned()
            } else if has_quiz_activity {
                "已完成".to_owned()
            } else {
                "未开始".to_owned()
            },
        },
    ]
}

fn training_learning_summary_response(
    summary: &TrainingLearningSummaryRecord,
    tasks: &[TrainingLearningTaskResp],
    weak_point_len: usize,
) -> TrainingLearningSummaryResp {
    let completed_weight = tasks
        .iter()
        .map(|task| match task.status.as_str() {
            "已完成" => 100,
            "进行中" => 50,
            _ => 0,
        })
        .sum::<i32>();
    let completion_rate = if tasks.is_empty() {
        0
    } else {
        completed_weight / tasks.len() as i32
    };
    let pending_task_count = tasks.iter().filter(|task| task.status != "已完成").count() as i32;
    let quiz_average_score = summary
        .latest_eval_average_score
        .map(|score| (score.clamp(0.0, 1.0) * 100.0).round() as i32)
        .or_else(|| {
            let total = summary.latest_eval_total_cases?;
            let passed = summary.latest_eval_passed_cases?;
            if total > 0 {
                Some(((passed as f64 / total as f64) * 100.0).round() as i32)
            } else {
                None
            }
        })
        .unwrap_or(0);

    TrainingLearningSummaryResp {
        completion_rate,
        pending_task_count,
        quiz_average_score,
        weak_point_count: weak_point_len.max(summary.weak_signal_count as usize) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_summary_uses_real_activity_signals() {
        let summary = TrainingLearningSummaryRecord {
            rag_trace_count: 2,
            feedback_count: 3,
            weak_signal_count: 2,
            quiz_wrong_count: 1,
            latest_eval_average_score: Some(0.86),
            latest_eval_total_cases: Some(20),
            latest_eval_passed_cases: Some(17),
        };
        let tasks = training_learning_tasks(&summary);
        let resp = training_learning_summary_response(&summary, &tasks, 2);

        assert_eq!(tasks[0].status, "进行中");
        assert_eq!(tasks[1].status, "已完成");
        assert_eq!(tasks[2].status, "待复习");
        assert_eq!(resp.completion_rate, 50);
        assert_eq!(resp.quiz_average_score, 86);
        assert_eq!(resp.weak_point_count, 2);
    }

    #[test]
    fn tenant_scope_lists_all_users_and_self_scope_defaults_to_current_user() {
        let tenant = normalize_learning_query(
            7,
            TrainingLearningQuery {
                scope: Some(" tenant ".to_owned()),
                user_id: Some(9),
            },
        )
        .unwrap();
        assert_eq!(tenant, ("tenant".to_owned(), None));

        let own = normalize_learning_query(
            7,
            TrainingLearningQuery {
                scope: None,
                user_id: None,
            },
        )
        .unwrap();
        assert_eq!(own, ("self".to_owned(), Some(7)));
    }
}
