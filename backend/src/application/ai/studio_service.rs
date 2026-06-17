use std::collections::HashMap;

use chrono::Utc;
use novex_model::ModelRoutePurpose;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{
    application::{
        ai::{
            knowledge_service::CitationResp,
            model_service::{ModelChatCommand, ModelChatMessage, ModelRuntimeService},
        },
        system::{ensure_max_chars, format_datetime, format_optional_datetime},
    },
    infrastructure::persistence::{
        ai_knowledge_repository::{AiKnowledgeRepository, ChunkRecord},
        ai_studio_repository::{
            AiStudioRepository, StudioActionRow, StudioArtifactRow, StudioArtifactSaveRecord,
        },
    },
    shared::{error::AppError, id::next_id},
};

const MIND_MAP_ACTION_CODE: &str = "mind_map.generate";
const DEFAULT_MIND_MAP_MAX_NODES: usize = 72;
const MAX_MIND_MAP_MAX_NODES: usize = 96;
const STUDIO_ARTIFACT_STATUS_ACTIVE: i16 = 1;
const STUDIO_ACTION_SURFACE_KNOWLEDGE: &str = "knowledge";
const MAX_STUDIO_TOPIC_CHARS: usize = 240;
const MAX_STUDIO_ACTION_CODE_CHARS: usize = 128;
const STUDIO_MIND_MAP_CHUNK_SCAN_LIMIT: i64 = 5_000;
const STUDIO_MIND_MAP_RETRIEVAL_LIMIT: usize = 4_096;

#[derive(Debug, Clone)]
pub struct StudioService {
    db: PgPool,
    repo: AiStudioRepository,
    knowledge_repo: AiKnowledgeRepository,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StudioMindMapContext {
    text: String,
    citation: CitationResp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StudioMindMapSection {
    title: String,
    first_index: usize,
    items: Vec<StudioMindMapSectionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StudioMindMapSectionItem {
    text: String,
    citation: CitationResp,
}

#[derive(Debug, Clone)]
struct StudioMindMapBuildResult {
    content_json: Value,
    answer_model_route: Option<String>,
    answer_model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StudioMindMapModelDraft {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    branches: Vec<StudioMindMapModelBranch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StudioMindMapModelBranch {
    #[serde(default)]
    label: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    citation_index: Option<usize>,
    #[serde(default)]
    children: Vec<StudioMindMapModelBranch>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioActionQuery {
    #[serde(default)]
    pub surface: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioArtifactGenerateCommand {
    #[serde(default)]
    pub action_code: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub session_id: Option<i64>,
    #[serde(default)]
    pub max_nodes: Option<usize>,
    #[serde(default)]
    pub answer_model_route_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioActionResp {
    pub id: i64,
    pub tenant_id: i64,
    pub code: String,
    pub name: String,
    pub description: String,
    pub surface: String,
    pub artifact_type: String,
    pub plugin_code: Option<String>,
    pub skill_code: Option<String>,
    pub permission_code: String,
    pub model_route_policy: Value,
    pub input_schema: Value,
    pub output_schema: Value,
    pub renderer: String,
    pub sort: i32,
    pub status: i16,
    pub metadata: Value,
    pub create_time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioArtifactResp {
    pub id: i64,
    pub tenant_id: i64,
    pub dataset_id: Option<i64>,
    pub session_id: Option<i64>,
    pub run_id: Option<i64>,
    pub rag_trace_id: Option<i64>,
    pub action_code: String,
    pub artifact_type: String,
    pub title: String,
    pub content_json: Value,
    pub content_text: String,
    pub source_snapshot: Value,
    pub citations: Vec<CitationResp>,
    pub version: i32,
    pub status: i16,
    pub metadata: Value,
    pub create_user: i64,
    pub create_time: String,
    pub update_time: String,
}

impl StudioService {
    pub fn new(db: PgPool) -> Self {
        Self {
            db: db.clone(),
            repo: AiStudioRepository::new(db.clone()),
            knowledge_repo: AiKnowledgeRepository::new(db),
        }
    }

    pub async fn list_actions(
        &self,
        tenant_id: i64,
        query: StudioActionQuery,
    ) -> Result<Vec<StudioActionResp>, AppError> {
        let surface = normalize_optional_studio_surface(query.surface)?;
        let rows = self
            .repo
            .list_actions(tenant_id, surface.as_deref())
            .await?;
        Ok(rows.into_iter().map(StudioActionResp::from).collect())
    }

    pub async fn list_dataset_artifacts(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
    ) -> Result<Vec<StudioArtifactResp>, AppError> {
        ensure_dataset_id(dataset_id)?;
        if !self
            .knowledge_repo
            .dataset_exists(tenant_id, dataset_id)
            .await?
        {
            return Err(AppError::NotFound);
        }
        let rows = self
            .repo
            .list_dataset_artifacts(tenant_id, user_id, dataset_id)
            .await?;
        Ok(rows.into_iter().map(StudioArtifactResp::from).collect())
    }

    pub async fn get_artifact(
        &self,
        tenant_id: i64,
        user_id: i64,
        artifact_id: i64,
    ) -> Result<StudioArtifactResp, AppError> {
        ensure_artifact_id(artifact_id)?;
        let row = self
            .repo
            .get_artifact(tenant_id, user_id, artifact_id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(StudioArtifactResp::from(row))
    }

    pub async fn delete_artifact(
        &self,
        tenant_id: i64,
        user_id: i64,
        artifact_id: i64,
    ) -> Result<i64, AppError> {
        ensure_artifact_id(artifact_id)?;
        let deleted = self
            .repo
            .soft_delete_artifact(tenant_id, user_id, artifact_id)
            .await?;
        if !deleted {
            return Err(AppError::NotFound);
        }
        Ok(artifact_id)
    }

    pub async fn generate_artifact(
        &self,
        tenant_id: i64,
        user_id: i64,
        dataset_id: i64,
        command: StudioArtifactGenerateCommand,
    ) -> Result<StudioArtifactResp, AppError> {
        ensure_dataset_id(dataset_id)?;
        let command = normalize_studio_artifact_generate_command(command)?;
        let action = self
            .repo
            .find_action(tenant_id, &command.action_code)
            .await?
            .ok_or_else(|| AppError::bad_request("Studio Action 不存在或已停用"))?;
        if action.surface != STUDIO_ACTION_SURFACE_KNOWLEDGE
            || action.code != MIND_MAP_ACTION_CODE
            || action.artifact_type != "mind_map"
        {
            return Err(AppError::bad_request("当前 Studio Action 暂不支持"));
        }
        if !self
            .knowledge_repo
            .dataset_exists(tenant_id, dataset_id)
            .await?
        {
            return Err(AppError::NotFound);
        }

        let max_nodes = command.max_nodes.unwrap_or(DEFAULT_MIND_MAP_MAX_NODES);
        let topic = studio_topic_or_default(&command.topic);
        let chunk_records = self
            .knowledge_repo
            .list_indexed_chunks(tenant_id, dataset_id, STUDIO_MIND_MAP_CHUNK_SCAN_LIMIT)
            .await?;
        let contexts = studio_mind_map_contexts(&topic, chunk_records, max_nodes);
        let citation_responses = contexts
            .iter()
            .map(|context| context.citation.clone())
            .collect::<Vec<_>>();
        let build_result = self
            .build_mind_map_content(
                tenant_id,
                &topic,
                &contexts,
                max_nodes,
                command.answer_model_route_id.as_deref(),
            )
            .await;
        let content_json = build_result.content_json.clone();
        let answer_strategy = content_json
            .pointer("/metadata/generation")
            .and_then(Value::as_str)
            .unwrap_or("studio_local_retrieval");
        let citations = serde_json::to_value(&citation_responses).unwrap_or_else(|_| json!([]));
        let now = Utc::now().naive_utc();
        let record = StudioArtifactSaveRecord {
            id: next_id(),
            tenant_id,
            dataset_id: Some(dataset_id),
            session_id: command.session_id,
            run_id: None,
            rag_trace_id: None,
            action_code: action.code.clone(),
            artifact_type: action.artifact_type.clone(),
            title: studio_artifact_title(&topic, &action),
            content_json: content_json.clone(),
            content_text: mind_map_content_text(&content_json),
            source_snapshot: json!({
                "source": "ai.studio",
                "datasetId": dataset_id,
                "sessionId": command.session_id,
                "topic": topic,
                "actionCode": action.code,
                "artifactType": action.artifact_type,
                "retrievalHitCount": contexts.len(),
                "answerStrategy": answer_strategy,
                "answerModelRoute": build_result.answer_model_route.or(command.answer_model_route_id),
                "answerModel": build_result.answer_model,
            }),
            citations,
            version: 1,
            status: STUDIO_ARTIFACT_STATUS_ACTIVE,
            metadata: json!({
                "renderer": action.renderer,
                "pluginCode": action.plugin_code,
                "skillCode": action.skill_code,
                "localRetrievalOnly": true,
            }),
            user_id,
            now,
        };
        let response = StudioArtifactResp::from_save_record(&record);
        self.repo.insert_artifact(&record).await?;

        Ok(response)
    }

    async fn build_mind_map_content(
        &self,
        tenant_id: i64,
        title: &str,
        contexts: &[StudioMindMapContext],
        max_nodes: usize,
        answer_model_route_id: Option<&str>,
    ) -> StudioMindMapBuildResult {
        let fallback = || StudioMindMapBuildResult {
            content_json: build_mind_map_from_contexts(title, contexts, max_nodes),
            answer_model_route: answer_model_route_id.map(str::to_owned),
            answer_model: None,
        };
        let sections = mind_map_sections_from_contexts(contexts);
        if sections.is_empty() {
            return fallback();
        }

        let branch_budget = max_nodes
            .saturating_sub(1)
            .checked_div(3)
            .unwrap_or(1)
            .max(1);
        let selected_sections = select_mind_map_sections(&sections, branch_budget.max(6));
        let command = studio_mind_map_chat_command(
            title,
            &selected_sections,
            max_nodes,
            answer_model_route_id,
        );
        let model_runtime = ModelRuntimeService::for_tenant(self.db.clone(), tenant_id);
        match model_runtime
            .chat_completion_for_purpose(ModelRoutePurpose::RagAnswer, command)
            .await
        {
            Ok(chat) => {
                match build_model_mind_map_content(
                    title,
                    &chat.answer,
                    &selected_sections,
                    max_nodes,
                ) {
                    Ok(content_json) => StudioMindMapBuildResult {
                        content_json,
                        answer_model_route: Some(chat.route_id),
                        answer_model: chat.model,
                    },
                    Err(err) => {
                        tracing::warn!(error = %err, "Studio mind map model output fell back to local generation");
                        fallback()
                    }
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "Studio mind map model generation fell back to local generation");
                fallback()
            }
        }
    }
}

pub fn build_deterministic_mind_map(
    title: &str,
    answer: &str,
    citations: &[CitationResp],
    max_nodes: usize,
) -> Value {
    let root_id = "root";
    let node_limit = max_nodes.max(1);
    let mut nodes = vec![json!({
        "id": root_id,
        "label": title.trim(),
        "summary": "核心主题",
        "level": 0,
        "citationRefs": []
    })];
    let mut edges = Vec::new();
    let citation_refs: Vec<Value> = citations
        .iter()
        .enumerate()
        .map(|(index, citation)| {
            json!({
                "id": format!("c{}", index + 1),
                "documentId": citation.document_id,
                "chunkId": citation.chunk_id,
                "pageNo": citation.page_no,
                "sectionPath": citation.section_path
            })
        })
        .collect();

    let mut topics: Vec<String> = answer
        .split(|ch| matches!(ch, '.' | '!' | '?' | '\n' | '。' | '！' | '？'))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.chars().take(80).collect::<String>())
        .collect();

    if topics.is_empty() && !answer.trim().is_empty() {
        topics.push(answer.trim().chars().take(80).collect());
    }
    if topics.is_empty() {
        topics.push(title.trim().to_owned());
    }

    for (index, topic) in topics
        .into_iter()
        .take(node_limit.saturating_sub(1))
        .enumerate()
    {
        if nodes.len() >= node_limit {
            break;
        }
        let node_id = format!("topic-{}", index + 1);
        let citation_ref = citation_refs
            .get(index % citation_refs.len().max(1))
            .and_then(|citation| citation["id"].as_str())
            .map(|id| vec![json!(id)])
            .unwrap_or_default();

        nodes.push(json!({
            "id": node_id,
            "label": compact_mind_map_label(&topic, 34),
            "summary": topic,
            "level": 1,
            "citationRefs": citation_ref
        }));
        edges.push(json!({
            "source": root_id,
            "target": node_id
        }));

        let detail_sentences = mind_map_sentences(&topic, 2);
        if nodes.len() < node_limit {
            let point_id = format!("{node_id}-point-1");
            let point_label = detail_sentences
                .first()
                .cloned()
                .unwrap_or_else(|| topic.clone());
            nodes.push(json!({
                "id": point_id,
                "label": compact_mind_map_label(&point_label, 34),
                "summary": topic,
                "level": 2,
                "citationRefs": citation_ref
            }));
            edges.push(json!({
                "source": node_id,
                "target": point_id
            }));
        }
    }

    json!({
        "title": title,
        "nodes": nodes,
        "edges": edges,
        "citations": citation_refs,
        "metadata": {
            "answerPreview": answer,
            "maxNodes": max_nodes
        }
    })
}

fn studio_mind_map_contexts(
    topic: &str,
    records: Vec<ChunkRecord>,
    max_nodes: usize,
) -> Vec<StudioMindMapContext> {
    let limit = max_nodes
        .saturating_mul(8)
        .max(DEFAULT_MIND_MAP_MAX_NODES * 4)
        .min(STUDIO_MIND_MAP_RETRIEVAL_LIMIT);
    let mut scored = records
        .into_iter()
        .filter(|record| is_mind_map_content_candidate(&record.content))
        .map(|record| {
            let score = studio_chunk_score(topic, &record);
            (score, record.document_id, record.chunk_index, record)
        })
        .collect::<Vec<_>>();

    let has_positive_score = scored.iter().any(|(score, _, _, _)| *score > 0);
    if has_positive_score {
        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
        });
    } else {
        scored.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.2.cmp(&right.2)));
    }

    let selected = if has_positive_score {
        scored.into_iter().take(limit).collect::<Vec<_>>()
    } else {
        take_evenly_spaced(scored, limit)
    };

    selected
        .into_iter()
        .map(|(_, _, _, record)| StudioMindMapContext {
            text: preview_chars(&record.content, 420),
            citation: CitationResp {
                document_id: record.document_id.to_string(),
                chunk_id: record.chunk_uid,
                page_no: record.page_no,
                section_path: section_path_from_value(&record.section_path),
            },
        })
        .collect()
}

fn take_evenly_spaced<T>(items: Vec<T>, limit: usize) -> Vec<T> {
    if limit == 0 || items.is_empty() {
        return Vec::new();
    }
    let len = items.len();
    if len <= limit {
        return items;
    }

    let last_index = len - 1;
    let last_slot = limit - 1;
    let target_indices = (0..limit)
        .map(|slot| slot * last_index / last_slot)
        .collect::<Vec<_>>();
    let mut target_cursor = 0;
    let mut selected = Vec::with_capacity(limit);
    for (index, item) in items.into_iter().enumerate() {
        if target_indices
            .get(target_cursor)
            .is_some_and(|target| *target == index)
        {
            selected.push(item);
            target_cursor += 1;
            if selected.len() >= limit {
                break;
            }
        }
    }
    selected
}

fn build_mind_map_from_contexts(
    title: &str,
    contexts: &[StudioMindMapContext],
    max_nodes: usize,
) -> Value {
    let sections = mind_map_sections_from_contexts(contexts);
    if sections.is_empty() {
        return build_deterministic_mind_map(title, title, &[], max_nodes);
    }

    let root_id = "root";
    let mut nodes = vec![json!({
        "id": root_id,
        "label": title.trim(),
        "summary": format!("基于 {} 个章节生成的结构化摘要", sections.len()),
        "level": 0,
        "citationRefs": []
    })];
    let mut edges = Vec::new();
    let mut citation_refs = Vec::new();
    let section_budget = max_nodes
        .saturating_sub(1)
        .checked_div(3)
        .unwrap_or(1)
        .max(1);
    let selected_sections = select_mind_map_sections(&sections, section_budget);

    for (index, section) in selected_sections.into_iter().enumerate() {
        if nodes.len() >= max_nodes.max(1) {
            break;
        }
        let node_id = format!("topic-{}", index + 1);
        let citation_id = push_mind_map_citation(&mut citation_refs, &section.items[0].citation);
        let sentences = mind_map_section_sentences(section, 4);
        let section_summary = sentences
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join("。");
        nodes.push(json!({
            "id": node_id,
            "label": preview_chars(&section.title, 48),
            "summary": preview_chars(&section_summary, 180),
            "level": 1,
            "citationRefs": [citation_id]
        }));
        edges.push(json!({
            "source": root_id,
            "target": node_id
        }));

        if nodes.len() < max_nodes.max(1) {
            let point_id = format!("{node_id}-point-1");
            let point_label = sentences
                .first()
                .cloned()
                .unwrap_or_else(|| section.title.clone());
            nodes.push(json!({
                "id": point_id,
                "label": compact_mind_map_label(&point_label, 34),
                "summary": preview_chars(&section_summary, 160),
                "level": 2,
                "citationRefs": [citation_id]
            }));
            edges.push(json!({
                "source": node_id,
                "target": point_id
            }));

            if nodes.len() < max_nodes.max(1) {
                if let Some(detail_label) = sentences.get(1).cloned() {
                    let detail_id = format!("{point_id}-detail-1");
                    nodes.push(json!({
                        "id": detail_id,
                        "label": compact_mind_map_label(&detail_label, 34),
                        "summary": sentences.get(2).cloned().unwrap_or_else(|| preview_chars(&section_summary, 160)),
                        "level": 3,
                        "citationRefs": [citation_id]
                    }));
                    edges.push(json!({
                        "source": point_id,
                        "target": detail_id
                    }));
                }
            }
        }
    }

    json!({
        "title": title,
        "nodes": nodes,
        "edges": edges,
        "citations": citation_refs,
        "metadata": {
            "generation": "studio_local_retrieval",
            "sectionCount": sections.len(),
            "maxNodes": max_nodes
        }
    })
}

fn studio_mind_map_chat_command(
    title: &str,
    sections: &[&StudioMindMapSection],
    max_nodes: usize,
    answer_model_route_id: Option<&str>,
) -> ModelChatCommand {
    let branch_budget = max_nodes
        .saturating_sub(1)
        .checked_div(3)
        .unwrap_or(1)
        .max(1);
    let section_outline = studio_mind_map_section_outline(sections);
    ModelChatCommand {
        conversation_id: None,
        route_id: answer_model_route_id.map(str::to_owned),
        messages: vec![
            ModelChatMessage {
                role: "system".to_owned(),
                content: format!(
                    "你是专业思维导图架构师和 NotebookLM 风格的资料总结器。只返回严格 JSON，不要 Markdown，不要解释。\
                    目标是根据用户总结方向，结合资料内在结构，尽可能完整地生成可视化思维导图。\
                    内部设计流程：先识别全文主线、关键矛盾、因果链、时间线、角色/概念关系和落地路径；\
                    再合并重复片段，过滤噪声，最后输出层级清晰的专业思维导图。\
                    规则：1) branches 最多 {branch_budget} 个；2) 总节点数不超过 {max_nodes}，但应充分利用预算覆盖重要信息；\
                    3) 深度最多 3 层，每个一级主题至少包含 2 个二级节点，重要二级节点继续展开三级细节；\
                    4) label 使用短主题词，中文不超过 14 个字，英文不超过 5 个词；\
                    5) summary 用一句话解释该节点价值，不超过 100 个字；6) 排除 OCR、图表代码、图片说明、乱码和无关广告文本；\
                    7) citationIndex 必须引用输入中的 S 编号；8) 如果资料覆盖多个阶段或对象，要按专业阅读顺序组织，而不是按原文顺序机械罗列；\
                    9) 人物成长史、履历、传记类内容必须按年份/阶段 + 关键事件组织，禁止把序位、序号、时间、日期、事件、备注等表头作为 label。\
                    JSON 形状：{{\"summary\":\"整体摘要\",\"branches\":[{{\"label\":\"主题\",\"summary\":\"说明\",\"citationIndex\":1,\"children\":[{{\"label\":\"子主题\",\"summary\":\"说明\",\"citationIndex\":1,\"children\":[]}}]}}]}}"
                ),
            },
            ModelChatMessage {
                role: "user".to_owned(),
                content: format!(
                    "用户总结方向：{}\n\n资料片段：\n{}\n\n返回严格 JSON。",
                    title.trim(),
                    section_outline
                ),
            },
        ],
        file_contexts: vec![],
        response_format: Some(json!({ "type": "json_object" })),
        temperature: Some(0.0),
        max_tokens: Some(3200),
        request_metadata: None,
        provider_call_context: None,
        provider_stream_sender: None,
    }
}

fn studio_mind_map_section_outline(sections: &[&StudioMindMapSection]) -> String {
    sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let bullets = mind_map_section_sentences(section, 5)
                .into_iter()
                .map(|sentence| format!("- {sentence}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!("[S{}] {}\n{}", index + 1, section.title, bullets)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_model_mind_map_content(
    title: &str,
    answer: &str,
    sections: &[&StudioMindMapSection],
    max_nodes: usize,
) -> Result<Value, String> {
    let draft = parse_model_mind_map_draft(answer)?;
    if draft.branches.is_empty() {
        return Err("model returned no branches".to_owned());
    }

    let root_id = "root";
    let root_summary = if draft.summary.trim().is_empty() {
        format!("基于 {} 个章节生成的结构化摘要", sections.len())
    } else {
        preview_chars(draft.summary.trim(), 160)
    };
    let mut nodes = vec![json!({
        "id": root_id,
        "label": title.trim(),
        "summary": root_summary,
        "level": 0,
        "citationRefs": []
    })];
    let mut edges = Vec::new();
    let mut citations = Vec::new();
    let mut citation_cache = HashMap::<usize, String>::new();
    let node_limit = max_nodes.max(1);

    for (index, branch) in draft.branches.iter().enumerate() {
        append_model_mind_map_branch(
            root_id,
            branch,
            1,
            &[index + 1],
            sections,
            &mut nodes,
            &mut edges,
            &mut citations,
            &mut citation_cache,
            node_limit,
        );
        if nodes.len() >= node_limit {
            break;
        }
    }

    if nodes.len() <= 1 {
        return Err("model branches did not produce valid nodes".to_owned());
    }

    Ok(json!({
        "title": title,
        "nodes": nodes,
        "edges": edges,
        "citations": citations,
        "metadata": {
            "generation": "studio_model_mind_map",
            "sectionCount": sections.len(),
            "maxNodes": max_nodes
        }
    }))
}

#[allow(clippy::too_many_arguments)]
fn append_model_mind_map_branch(
    parent_id: &str,
    branch: &StudioMindMapModelBranch,
    level: i32,
    path: &[usize],
    sections: &[&StudioMindMapSection],
    nodes: &mut Vec<Value>,
    edges: &mut Vec<Value>,
    citations: &mut Vec<Value>,
    citation_cache: &mut HashMap<usize, String>,
    node_limit: usize,
) {
    if nodes.len() >= node_limit || level > 3 {
        return;
    }
    let label = meaningful_mind_map_label(&branch.label, &branch.summary, 28);
    if label.chars().count() < 2 {
        return;
    }
    let node_id = model_mind_map_node_id(level, path);
    let raw_summary = if branch.summary.trim().is_empty() {
        String::new()
    } else {
        preview_chars(branch.summary.trim(), 120)
    };
    let summary = mind_map_node_summary(&label, &raw_summary);
    let citation_refs =
        model_mind_map_citation_refs(branch.citation_index, sections, citations, citation_cache);

    nodes.push(json!({
        "id": node_id,
        "label": label.clone(),
        "summary": summary.clone(),
        "level": level,
        "citationRefs": citation_refs.clone()
    }));
    edges.push(json!({
        "source": parent_id,
        "target": node_id
    }));

    if level >= 3 {
        return;
    }
    if level == 2
        && branch.children.is_empty()
        && !raw_summary.is_empty()
        && nodes.len() < node_limit
    {
        let detail_label = compact_mind_map_label(&raw_summary, 28);
        if detail_label != label && detail_label.chars().count() >= 2 {
            if let Some(parent_node) = nodes.last_mut() {
                parent_node["summary"] = Value::String(String::new());
            }
            let mut detail_path = path.to_vec();
            detail_path.push(1);
            let detail_id = model_mind_map_node_id(3, &detail_path);
            let detail_summary = mind_map_node_summary(&detail_label, &raw_summary);
            nodes.push(json!({
                "id": detail_id,
                "label": detail_label,
                "summary": detail_summary,
                "level": 3,
                "citationRefs": citation_refs
            }));
            edges.push(json!({
                "source": node_id,
                "target": detail_id
            }));
            return;
        }
    }
    for (index, child) in branch.children.iter().enumerate() {
        let mut child_path = path.to_vec();
        child_path.push(index + 1);
        append_model_mind_map_branch(
            &node_id,
            child,
            level + 1,
            &child_path,
            sections,
            nodes,
            edges,
            citations,
            citation_cache,
            node_limit,
        );
        if nodes.len() >= node_limit {
            break;
        }
    }
}

fn model_mind_map_node_id(level: i32, path: &[usize]) -> String {
    match level {
        1 => format!("topic-{}", path[0]),
        2 => format!("topic-{}-point-{}", path[0], path[1]),
        _ => format!("topic-{}-point-{}-detail-{}", path[0], path[1], path[2]),
    }
}

fn model_mind_map_citation_refs(
    citation_index: Option<usize>,
    sections: &[&StudioMindMapSection],
    citations: &mut Vec<Value>,
    citation_cache: &mut HashMap<usize, String>,
) -> Vec<Value> {
    let Some(section_index) = citation_index.and_then(|index| index.checked_sub(1)) else {
        return Vec::new();
    };
    let Some(section) = sections.get(section_index) else {
        return Vec::new();
    };
    let Some(item) = section.items.first() else {
        return Vec::new();
    };
    let citation_id = citation_cache
        .entry(section_index)
        .or_insert_with(|| push_mind_map_citation(citations, &item.citation))
        .clone();
    vec![json!(citation_id)]
}

fn parse_model_mind_map_draft(answer: &str) -> Result<StudioMindMapModelDraft, String> {
    let json_text = extract_json_object(answer)
        .ok_or_else(|| "model answer did not contain JSON".to_owned())?;
    serde_json::from_str::<StudioMindMapModelDraft>(json_text)
        .map_err(|err| format!("invalid mind map JSON: {err}"))
}

fn extract_json_object(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end < start {
        return None;
    }
    Some(&trimmed[start..=end])
}

fn studio_chunk_score(topic: &str, record: &ChunkRecord) -> i32 {
    let haystack = format!(
        "{}\n{}\n{}",
        record.semantic_search_text,
        record.content,
        section_path_from_value(&record.section_path).join(" ")
    )
    .to_lowercase();
    topic
        .split(|ch: char| {
            ch.is_whitespace() || matches!(ch, '-' | '_' | '/' | '|' | '，' | ',' | '。')
        })
        .map(str::trim)
        .filter(|token| token.chars().count() >= 2)
        .map(|token| haystack.matches(&token.to_lowercase()).count().min(5) as i32)
        .sum()
}

fn mind_map_context_label(context: &StudioMindMapContext) -> String {
    let section_label = context
        .citation
        .section_path
        .last()
        .map(|section| section.trim())
        .filter(|section| !section.is_empty())
        .map(|section| preview_chars(section, 48));
    if let Some(label) = section_label {
        if !is_mind_map_dimension_heading(&label) {
            return label;
        }
    }
    mind_map_text_label(&context.text, 48)
}

fn mind_map_sections_from_contexts(contexts: &[StudioMindMapContext]) -> Vec<StudioMindMapSection> {
    let mut sections = Vec::<StudioMindMapSection>::new();
    for (index, context) in contexts.iter().enumerate() {
        if !is_mind_map_content_candidate(&context.text) {
            continue;
        }
        let title = mind_map_context_label(context);
        let item = StudioMindMapSectionItem {
            text: context.text.clone(),
            citation: context.citation.clone(),
        };
        if let Some(section) = sections.iter_mut().find(|section| section.title == title) {
            section.items.push(item);
        } else {
            sections.push(StudioMindMapSection {
                title,
                first_index: index,
                items: vec![item],
            });
        }
    }
    sections
        .into_iter()
        .filter(|section| !mind_map_section_sentences(section, 1).is_empty())
        .collect()
}

fn select_mind_map_sections(
    sections: &[StudioMindMapSection],
    budget: usize,
) -> Vec<&StudioMindMapSection> {
    let budget = budget.max(1);
    let mut selected = Vec::<usize>::new();
    if !sections.is_empty() {
        selected.push(0);
    }

    let mut ranked = (0..sections.len()).collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        sections[*right]
            .items
            .len()
            .cmp(&sections[*left].items.len())
            .then_with(|| {
                sections[*left]
                    .first_index
                    .cmp(&sections[*right].first_index)
            })
    });
    for index in ranked {
        if selected.len() >= budget {
            break;
        }
        if !selected.contains(&index) {
            selected.push(index);
        }
    }

    selected.sort_by_key(|index| sections[*index].first_index);
    selected
        .into_iter()
        .filter_map(|index| sections.get(index))
        .collect()
}

fn mind_map_section_sentences(section: &StudioMindMapSection, limit: usize) -> Vec<String> {
    let mut sentences = Vec::new();
    let prefer_cjk = contains_cjk(&section.title);
    for item in &section.items {
        for sentence in
            mind_map_sentences_with_language(&item.text, limit.saturating_mul(2), prefer_cjk)
        {
            if sentence == section.title
                || sentence.chars().count() < 8
                || sentences.iter().any(|existing| existing == &sentence)
            {
                continue;
            }
            sentences.push(sentence);
            if sentences.len() >= limit {
                return sentences;
            }
        }
    }
    sentences
}

fn first_sentence_label(text: &str, limit: usize) -> String {
    let candidate = text
        .split(|ch| matches!(ch, '.' | '!' | '?' | '\n' | '。' | '！' | '？' | '；' | ';'))
        .map(str::trim)
        .find(|part| !part.is_empty())
        .unwrap_or("要点");
    preview_chars(candidate, limit)
}

fn compact_mind_map_label(text: &str, limit: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = normalized
        .trim_matches(|ch: char| {
            ch.is_whitespace() || matches!(ch, '。' | '，' | ',' | '.' | '；' | ';')
        })
        .to_owned();
    if normalized.is_empty() {
        return "要点".to_owned();
    }
    if let Some(table_label) = mind_map_table_row_label(&normalized, limit) {
        return table_label;
    }

    let clauses = normalized
        .split(|ch| matches!(ch, '。' | '，' | ',' | '.' | '；' | ';' | '：' | ':'))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let candidate = clauses
        .iter()
        .map(|clause| trim_mind_map_discourse_prefix(clause))
        .find(|clause| is_compact_label_clause(clause))
        .unwrap_or(normalized.as_str());
    let label = preview_chars(candidate, limit);
    if is_mind_map_dimension_heading(&label) || is_mind_map_table_scaffold_label(&label) {
        "要点".to_owned()
    } else {
        label
    }
}

fn meaningful_mind_map_label(label: &str, summary: &str, limit: usize) -> String {
    let compact_label = compact_mind_map_label(label, limit);
    if compact_label != "要点" && !is_mind_map_dimension_heading(&compact_label) {
        return compact_label;
    }

    let compact_summary = compact_mind_map_label(summary, limit);
    if compact_summary != "要点" && !is_mind_map_dimension_heading(&compact_summary) {
        return compact_summary;
    }

    "关键阶段".to_owned()
}

fn mind_map_text_label(text: &str, limit: usize) -> String {
    if let Some(table_label) = mind_map_table_row_label(text, limit) {
        return table_label;
    }
    for sentence in mind_map_sentences_with_language(text, 8, true) {
        let label = compact_mind_map_label(&sentence, limit);
        if label != "要点" && !is_mind_map_dimension_heading(&label) {
            return label;
        }
    }

    let label = first_sentence_label(text, limit);
    if is_mind_map_dimension_heading(&label) || is_mind_map_table_scaffold_label(&label) {
        "关键阶段".to_owned()
    } else {
        label
    }
}

fn mind_map_table_row_label(text: &str, limit: usize) -> Option<String> {
    for line in text.lines() {
        let Some(cells) = markdown_table_cells(line) else {
            continue;
        };
        if cells.len() < 2 || is_markdown_table_alignment_row(&cells) {
            continue;
        }
        let meaningful = cells
            .into_iter()
            .map(|cell| normalize_mind_map_table_cell(&cell))
            .filter(|cell| {
                !cell.is_empty()
                    && !is_mind_map_dimension_heading(cell)
                    && !is_mind_map_sequence_cell(cell)
                    && !is_markdown_table_alignment_cell(cell)
            })
            .collect::<Vec<_>>();
        if meaningful.is_empty() {
            continue;
        }
        let label = preview_chars(&meaningful.join(" "), limit);
        if !is_mind_map_table_scaffold_label(&label) && !is_mind_map_dimension_heading(&label) {
            return Some(label);
        }
    }
    None
}

fn markdown_table_cells(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return None;
    }
    let cells = trimmed
        .trim_matches('|')
        .split('|')
        .map(normalize_mind_map_table_cell)
        .filter(|cell| !cell.is_empty())
        .collect::<Vec<_>>();
    (cells.len() >= 2).then_some(cells)
}

fn normalize_mind_map_table_cell(cell: &str) -> String {
    cell.trim()
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '*' | '#' | '`' | '|' | '。' | '，' | ',' | '.' | '；' | ';' | '：' | ':'
                )
        })
        .trim()
        .to_owned()
}

fn is_markdown_table_alignment_row(cells: &[String]) -> bool {
    cells
        .iter()
        .all(|cell| is_markdown_table_alignment_cell(cell))
}

fn is_markdown_table_alignment_cell(cell: &str) -> bool {
    let trimmed = cell.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| matches!(ch, '-' | ':' | ' ' | '\t'))
}

fn is_mind_map_sequence_cell(text: &str) -> bool {
    let normalized = mind_map_text_key(text);
    if normalized.is_empty() {
        return false;
    }
    normalized.chars().all(|ch| ch.is_ascii_digit())
        || (normalized.starts_with('第')
            && normalized.chars().skip(1).all(|ch| {
                ch.is_ascii_digit()
                    || matches!(
                        ch,
                        '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' | '十'
                    )
            }))
}

fn is_mind_map_dimension_heading(text: &str) -> bool {
    let key = mind_map_text_key(text);
    matches!(
        key.as_str(),
        "序位"
            | "序号"
            | "编号"
            | "排名"
            | "时间"
            | "日期"
            | "年份"
            | "事件"
            | "事项"
            | "备注"
            | "说明"
            | "描述"
            | "内容"
            | "阶段"
            | "时期"
            | "维度"
            | "字段"
    )
}

fn is_mind_map_table_scaffold_label(text: &str) -> bool {
    markdown_table_cells(text).is_some_and(|cells| {
        is_markdown_table_alignment_row(&cells)
            || cells
                .iter()
                .all(|cell| is_mind_map_dimension_heading(cell) || is_mind_map_sequence_cell(cell))
    })
}

fn mind_map_node_summary(label: &str, summary: &str) -> String {
    let summary = summary.trim();
    if summary.is_empty() {
        return String::new();
    }
    let summary = preview_chars(summary, 120);
    if mind_map_text_key(&summary) == mind_map_text_key(label) {
        String::new()
    } else {
        summary
    }
}

fn mind_map_text_key(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn trim_mind_map_discourse_prefix(text: &str) -> &str {
    let mut trimmed = text.trim();
    for prefix in [
        "另外",
        "同时",
        "但是",
        "不过",
        "因此",
        "所以",
        "其中",
        "其实",
        "前文已经说了",
        "从这个意义上说",
        "一方面",
        "另一方面",
        "但",
        "而",
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            trimmed = rest.trim_start_matches(|ch: char| {
                ch.is_whitespace() || matches!(ch, '，' | ',' | '：' | ':')
            });
            break;
        }
    }
    trimmed
}

fn is_compact_label_clause(text: &str) -> bool {
    let char_count = text.chars().count();
    if char_count < 4 {
        return false;
    }
    if is_mind_map_dimension_heading(text) || is_mind_map_table_scaffold_label(text) {
        return false;
    }
    if text.ends_with("而言") || text.ends_with("来说") {
        return false;
    }
    if text.starts_with('我') || text.starts_with("我们") {
        return false;
    }
    if text.starts_with('在') && text.contains("节点") {
        return false;
    }
    if text.starts_with('在')
        && char_count <= 12
        && (text.ends_with('上') || text.ends_with('中') || text.ends_with('里'))
    {
        return false;
    }
    true
}

fn mind_map_sentences(text: &str, limit: usize) -> Vec<String> {
    mind_map_sentences_with_language(text, limit, false)
}

fn mind_map_sentences_with_language(text: &str, limit: usize, prefer_cjk: bool) -> Vec<String> {
    text.split(|ch| matches!(ch, '.' | '!' | '?' | '\n' | '。' | '！' | '？' | '；' | ';'))
        .map(str::trim)
        .filter(|part| is_mind_map_sentence_candidate(part, prefer_cjk))
        .map(|part| preview_chars(part, 88))
        .take(limit)
        .collect()
}

fn is_mind_map_content_candidate(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("![](")
        || trimmed.starts_with("<details>")
        || trimmed.contains("<summary>text_image</summary>")
        || trimmed.contains("</details>")
        || trimmed.contains("```")
        || trimmed.contains("subgraph ")
        || trimmed.contains("graph TD")
        || trimmed.contains("-->")
        || trimmed.contains("[\"")
        || has_unbalanced_mind_map_quotes(trimmed)
    {
        return false;
    }
    let meaningful_chars = trimmed
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .count();
    meaningful_chars >= 12
}

fn is_mind_map_sentence_candidate(text: &str, prefer_cjk: bool) -> bool {
    let trimmed = text.trim();
    let cjk_count = trimmed
        .chars()
        .filter(|ch| ('\u{4e00}'..='\u{9fff}').contains(ch))
        .count();
    let ascii_alnum_count = trimmed
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .count();
    if is_mind_map_dimension_heading(trimmed) || is_mind_map_table_scaffold_label(trimmed) {
        return false;
    }
    is_mind_map_content_candidate(trimmed)
        && !trimmed.contains("images/")
        && !trimmed.contains("text_image")
        && !trimmed.contains("网钉科技有限公司")
        && !trimmed.contains("subgraph")
        && !trimmed.contains("[\"")
        && !trimmed.contains("\"]")
        && !trimmed.contains("-->")
        && !trimmed.contains("classDef")
        && !trimmed.contains("style ")
        && !has_unbalanced_mind_map_quotes(trimmed)
        && !(prefer_cjk && cjk_count < 3 && ascii_alnum_count > 20)
}

fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
}

fn has_unbalanced_mind_map_quotes(text: &str) -> bool {
    text.matches('“').count() != text.matches('”').count()
        || text.matches('「').count() != text.matches('」').count()
        || text.matches('『').count() != text.matches('』').count()
}

fn section_path_from_value(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn push_mind_map_citation(citations: &mut Vec<Value>, citation: &CitationResp) -> String {
    let id = format!("c{}", citations.len() + 1);
    citations.push(json!({
        "id": id,
        "documentId": citation.document_id,
        "chunkId": citation.chunk_id,
        "pageNo": citation.page_no,
        "sectionPath": citation.section_path
    }));
    id
}

fn mind_map_content_text(content: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(title) = content.get("title").and_then(Value::as_str) {
        parts.push(title.to_owned());
    }
    if let Some(nodes) = content.get("nodes").and_then(Value::as_array) {
        for node in nodes.iter().take(12) {
            if let Some(label) = node.get("label").and_then(Value::as_str) {
                parts.push(label.to_owned());
            }
        }
    }
    preview_chars(&parts.join("\n"), 2000)
}

fn normalize_studio_artifact_generate_command(
    mut command: StudioArtifactGenerateCommand,
) -> Result<StudioArtifactGenerateCommand, AppError> {
    command.action_code = command.action_code.trim().to_owned();
    if command.action_code.is_empty() {
        command.action_code = MIND_MAP_ACTION_CODE.to_owned();
    }
    ensure_max_chars(
        "Studio Action",
        &command.action_code,
        MAX_STUDIO_ACTION_CODE_CHARS,
    )?;
    if !command
        .action_code
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return Err(AppError::bad_request("Studio Action 不合法"));
    }
    command.topic = command.topic.trim().to_owned();
    ensure_max_chars("主题", &command.topic, MAX_STUDIO_TOPIC_CHARS)?;
    if matches!(command.session_id, Some(value) if value <= 0) {
        return Err(AppError::bad_request("会话 ID 不合法"));
    }
    command.max_nodes = Some(
        command
            .max_nodes
            .unwrap_or(DEFAULT_MIND_MAP_MAX_NODES)
            .clamp(1, MAX_MIND_MAP_MAX_NODES),
    );
    command.answer_model_route_id =
        normalize_optional_studio_route_id(command.answer_model_route_id)?;
    Ok(command)
}

fn normalize_optional_studio_surface(surface: Option<String>) -> Result<Option<String>, AppError> {
    let surface = surface
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(surface) = &surface {
        ensure_max_chars("Studio Surface", surface, 64)?;
        if !surface
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            return Err(AppError::bad_request("Studio Surface 不合法"));
        }
    }
    Ok(surface)
}

fn normalize_optional_studio_route_id(
    route_id: Option<String>,
) -> Result<Option<String>, AppError> {
    let route_id = route_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(route_id) = &route_id {
        ensure_max_chars("模型路由", route_id, 128)?;
    }
    Ok(route_id)
}

fn ensure_dataset_id(dataset_id: i64) -> Result<(), AppError> {
    if dataset_id <= 0 {
        Err(AppError::bad_request("知识库 ID 不合法"))
    } else {
        Ok(())
    }
}

fn ensure_artifact_id(artifact_id: i64) -> Result<(), AppError> {
    if artifact_id <= 0 {
        Err(AppError::bad_request("Artifact ID 不合法"))
    } else {
        Ok(())
    }
}

fn studio_topic_or_default(topic: &str) -> String {
    if topic.trim().is_empty() {
        "知识库思维导图".to_owned()
    } else {
        topic.trim().to_owned()
    }
}

fn studio_artifact_title(topic: &str, action: &StudioActionRow) -> String {
    preview_chars(&format!("{topic} - {}", action.name), 120)
}

fn preview_chars(text: &str, limit: usize) -> String {
    let mut value = text.trim().chars().take(limit).collect::<String>();
    if text.trim().chars().count() > limit {
        value.push('…');
    }
    value
}

fn studio_citations(value: Value) -> Vec<CitationResp> {
    serde_json::from_value(value).unwrap_or_default()
}

impl StudioActionResp {
    fn from(row: StudioActionRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            code: row.code,
            name: row.name,
            description: row.description.unwrap_or_default(),
            surface: row.surface,
            artifact_type: row.artifact_type,
            plugin_code: row.plugin_code,
            skill_code: row.skill_code,
            permission_code: row.permission_code,
            model_route_policy: row.model_route_policy,
            input_schema: row.input_schema,
            output_schema: row.output_schema,
            renderer: row.renderer,
            sort: row.sort,
            status: row.status,
            metadata: row.metadata,
            create_time: format_datetime(row.create_time),
        }
    }
}

impl StudioArtifactResp {
    fn from_save_record(record: &StudioArtifactSaveRecord) -> Self {
        Self {
            id: record.id,
            tenant_id: record.tenant_id,
            dataset_id: record.dataset_id,
            session_id: record.session_id,
            run_id: record.run_id,
            rag_trace_id: record.rag_trace_id,
            action_code: record.action_code.clone(),
            artifact_type: record.artifact_type.clone(),
            title: record.title.clone(),
            content_json: record.content_json.clone(),
            content_text: record.content_text.clone(),
            source_snapshot: record.source_snapshot.clone(),
            citations: studio_citations(record.citations.clone()),
            version: record.version,
            status: record.status,
            metadata: record.metadata.clone(),
            create_user: record.user_id,
            create_time: format_datetime(record.now),
            update_time: format_datetime(record.now),
        }
    }
}

impl From<StudioArtifactRow> for StudioArtifactResp {
    fn from(row: StudioArtifactRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            dataset_id: row.dataset_id,
            session_id: row.session_id,
            run_id: row.run_id,
            rag_trace_id: row.rag_trace_id,
            action_code: row.action_code,
            artifact_type: row.artifact_type,
            title: row.title,
            content_json: row.content_json,
            content_text: row.content_text,
            source_snapshot: row.source_snapshot,
            citations: studio_citations(row.citations),
            version: row.version,
            status: row.status,
            metadata: row.metadata,
            create_user: row.create_user,
            create_time: format_datetime(row.create_time),
            update_time: format_optional_datetime(row.update_time),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn citation(document_id: &str, chunk_id: &str, page_no: Option<i32>) -> CitationResp {
        CitationResp {
            document_id: document_id.to_owned(),
            chunk_id: chunk_id.to_owned(),
            page_no,
            section_path: vec!["Guide".to_owned()],
        }
    }

    fn section_citation(section: &str, chunk_id: &str) -> CitationResp {
        CitationResp {
            document_id: "20".to_owned(),
            chunk_id: chunk_id.to_owned(),
            page_no: None,
            section_path: vec![section.to_owned()],
        }
    }

    fn chunk_record(section: &str, index: i32, content: String) -> ChunkRecord {
        ChunkRecord {
            id: i64::from(index),
            document_id: 20,
            chunk_uid: format!("20:{index}"),
            chunk_index: index,
            content: content.clone(),
            semantic_search_text: content,
            token_count: 64,
            citation: serde_json::json!({}),
            segment_type: "text".to_owned(),
            segment_index: index,
            page_no: None,
            section_path: serde_json::json!([section]),
            content_role: "body".to_owned(),
            display_capability: "text".to_owned(),
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn studio_service_builds_cited_mind_map_json_from_answer() {
        let content = build_deterministic_mind_map(
            "Training Handbook",
            "Onboarding covers policy basics. Security training covers incident response.",
            &[
                citation("20", "20:0", Some(3)),
                citation("21", "21:2", None),
            ],
            8,
        );

        assert_eq!(content["title"], "Training Handbook");
        let nodes = content["nodes"]
            .as_array()
            .expect("nodes should be an array");
        assert!(
            nodes.iter().any(|node| node["id"] == "root"),
            "mind map should include a root node"
        );
        assert!(
            nodes.iter().any(|node| node["citationRefs"]
                .as_array()
                .is_some_and(|refs| !refs.is_empty())),
            "at least one node should carry citation refs"
        );
        let edges = content["edges"]
            .as_array()
            .expect("edges should be an array");
        assert!(!edges.is_empty(), "mind map should include root edges");
        let citations = content["citations"]
            .as_array()
            .expect("citations should be an array");
        assert_eq!(citations.len(), 2);
    }

    #[test]
    fn studio_service_builds_nested_mind_map_levels_from_contexts() {
        let content = build_mind_map_from_contexts(
            "Training Handbook",
            &[StudioMindMapContext {
                text: "Security training covers incident response. Teams triage incidents before escalation. Critical incidents require manager review.".to_owned(),
                citation: citation("20", "20:0", Some(3)),
            }],
            6,
        );

        let nodes = content["nodes"]
            .as_array()
            .expect("nodes should be an array");
        assert!(
            nodes.iter().any(|node| node["level"] == 2),
            "mind map should include second-level nodes"
        );
        assert!(
            nodes.iter().any(|node| node["level"] == 3),
            "mind map should include third-level nodes"
        );

        let edges = content["edges"]
            .as_array()
            .expect("edges should be an array");
        assert!(
            edges
                .iter()
                .any(|edge| edge["source"] == "topic-1" && edge["target"] == "topic-1-point-1"),
            "level 1 topic should connect to a level 2 point"
        );
        assert!(
            edges.iter().any(|edge| edge["source"] == "topic-1-point-1"
                && edge["target"] == "topic-1-point-1-detail-1"),
            "level 2 point should connect to a level 3 detail"
        );
    }

    #[test]
    fn studio_service_groups_mind_map_by_sections_and_filters_noise() {
        let content = build_mind_map_from_contexts(
            "未命名的笔记本",
            &[
                StudioMindMapContext {
                    text: "钉钉的动物园形象钉三多，是一只尖尾雨燕。它强调长期飞行与不落地的隐喻。"
                        .to_owned(),
                    citation: section_citation("楔：钉钉是一只雨燕", "20:0"),
                },
                StudioMindMapContext {
                    text: "我在项目中经历了从 0 到 1 的迭代，并见证项目进入暮年运营阶段。"
                        .to_owned(),
                    citation: section_citation("楔：钉钉是一只雨燕", "20:1"),
                },
                StudioMindMapContext {
                    text: "![](images/screenshot.jpg)".to_owned(),
                    citation: section_citation("ONE是一个怎样的项目", "20:2"),
                },
                StudioMindMapContext {
                    text: "<details><summary>text_image</summary>工作 网钉科技有限公司</details>"
                        .to_owned(),
                    citation: section_citation("ONE是一个怎样的项目", "20:3"),
                },
                StudioMindMapContext {
                    text: "ONE 是无招回归后第一个主推的 AI 原生项目，发布会后 DAU 达到阶段性高点。"
                        .to_owned(),
                    citation: section_citation("ONE是一个怎样的项目", "20:4"),
                },
                StudioMindMapContext {
                    text: "“给ADHD用的 办公工M效率。".to_owned(),
                    citation: section_citation("老板 v.s. 员工", "20:5"),
                },
                StudioMindMapContext {
                    text: "外部环境变化让企业协同产品重新寻找 AI 原生入口。".to_owned(),
                    citation: section_citation("外部环境：2025 年的风向", "20:6"),
                },
            ],
            16,
        );

        let nodes = content["nodes"]
            .as_array()
            .expect("nodes should be an array");
        let level_one_labels = nodes
            .iter()
            .filter(|node| node["level"] == 1)
            .filter_map(|node| node["label"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            level_one_labels
                .iter()
                .filter(|label| **label == "楔：钉钉是一只雨燕")
                .count(),
            1,
            "same section should not be repeated as multiple top-level branches"
        );
        assert!(
            level_one_labels.contains(&"ONE是一个怎样的项目"),
            "section with real prose should survive noise filtering"
        );
        assert!(
            !nodes
                .iter()
                .any(|node| node["label"]
                    .as_str()
                    .is_some_and(|label| label.contains("images/")
                        || label.contains("text_image")
                        || label.contains("网钉科技有限公司")
                        || label.contains("ADHD"))),
            "image and OCR scaffolding should not become mind map nodes"
        );
    }

    #[test]
    fn studio_mind_map_fallback_uses_event_labels_instead_of_table_headers() {
        let content = build_mind_map_from_contexts(
            "人物成长史",
            &[StudioMindMapContext {
                text: "| 序位* | 时间 | 事件 |\n| --- | --- | --- |\n| 1 | 1982年 | 出生于杭州，后来进入产品领域。 |\n| 2 | 2015年 | 带领团队完成关键产品发布。 |"
                    .to_owned(),
                citation: section_citation("序位*", "20:70"),
            }],
            8,
        );

        let nodes = content["nodes"]
            .as_array()
            .expect("nodes should be an array");
        let level_one_labels = nodes
            .iter()
            .filter(|node| node["level"] == 1)
            .filter_map(|node| node["label"].as_str())
            .collect::<Vec<_>>();

        assert!(
            level_one_labels
                .iter()
                .all(|label| *label != "序位*" && *label != "时间"),
            "table headers should not become visible mind-map dimensions"
        );
        assert!(
            level_one_labels
                .iter()
                .any(|label| label.contains("1982年") && label.contains("出生")),
            "person timeline nodes should expose the actual time and event"
        );
    }

    #[test]
    fn studio_mind_map_labels_are_compact_for_visual_nodes() {
        assert_eq!(
            compact_mind_map_label(
                "在钉钉上，老板和员工是截然不同的画像。老板需要督办员工完成任务。",
                34
            ),
            "老板和员工是截然不同的画像"
        );
        assert_eq!(
            compact_mind_map_label("另外，无招的回归本身也是一个非常奇特的变量。", 34),
            "无招的回归本身也是一个非常奇特的变量"
        );
        assert_eq!(
            compact_mind_map_label(
                "前文已经说了，ONE 的目标是服务大 DAU 且高频的用户操作。",
                34
            ),
            "ONE 的目标是服务大 DAU 且高频的用户操作"
        );
        assert_eq!(
            compact_mind_map_label("从这个意义上说，无招既是钉钉的大老板。", 34),
            "无招既是钉钉的大老板"
        );
        assert!(
            compact_mind_map_label(
                "虽然就从工作时间性价比和人性化程度而言，钉钉都不值得看好。",
                18
            )
            .chars()
            .count()
                <= 18
        );
    }

    #[test]
    fn studio_service_builds_mind_map_from_model_json() {
        let sections = vec![
            StudioMindMapSection {
                title: "组织转型".to_owned(),
                first_index: 0,
                items: vec![StudioMindMapSectionItem {
                    text: "组织在 AI 原生阶段重构协作入口。".to_owned(),
                    citation: section_citation("组织转型", "20:10"),
                }],
            },
            StudioMindMapSection {
                title: "产品设计".to_owned(),
                first_index: 1,
                items: vec![StudioMindMapSectionItem {
                    text: "产品通过统一工作台承接群聊、待办和审批。".to_owned(),
                    citation: section_citation("产品设计", "20:20"),
                }],
            },
        ];
        let section_refs = sections.iter().collect::<Vec<_>>();
        let content = build_model_mind_map_content(
            "未命名的笔记本",
            r#"```json
            {
              "summary": "围绕组织转型和产品设计提炼主线",
              "branches": [
                {
                  "label": "组织转型",
                  "summary": "AI 原生阶段改变协作入口",
                  "citationIndex": 1,
                  "children": [
                    {
                      "label": "协作入口",
                      "summary": "工作信息需要被统一承接",
                      "citationIndex": 2,
                      "children": [
                        {
                          "label": "统一工作台",
                          "summary": "群聊、待办和审批进入一页处理",
                          "citationIndex": 2,
                          "children": []
                        }
                      ]
                    }
                  ]
                }
              ]
            }
            ```"#,
            &section_refs,
            8,
        )
        .expect("model JSON should build mind map content");

        let nodes = content["nodes"]
            .as_array()
            .expect("nodes should be an array");
        assert!(nodes.iter().any(|node| node["level"] == 3));
        assert!(
            nodes.iter().any(|node| node["label"] == "统一工作台"
                && node["citationRefs"]
                    .as_array()
                    .is_some_and(|refs| !refs.is_empty())),
            "model nodes should preserve citation refs"
        );
        assert_eq!(content["metadata"]["generation"], "studio_model_mind_map");
    }

    #[test]
    fn studio_service_replaces_model_table_header_labels_with_stage_events() {
        let sections = vec![StudioMindMapSection {
            title: "序位*".to_owned(),
            first_index: 0,
            items: vec![StudioMindMapSectionItem {
                text: "1982年出生于杭州，随后进入产品领域。".to_owned(),
                citation: section_citation("序位*", "20:80"),
            }],
        }];
        let section_refs = sections.iter().collect::<Vec<_>>();
        let content = build_model_mind_map_content(
            "人物成长史",
            r#"{
              "summary": "按人物成长阶段整理关键事件",
              "branches": [
                {
                  "label": "时间",
                  "summary": "1982年出生于杭州，随后进入产品领域",
                  "citationIndex": 1,
                  "children": []
                }
              ]
            }"#,
            &section_refs,
            6,
        )
        .expect("model JSON should build mind map content");

        let nodes = content["nodes"]
            .as_array()
            .expect("nodes should be an array");
        let level_one_label = nodes
            .iter()
            .find(|node| node["level"] == 1)
            .and_then(|node| node["label"].as_str())
            .expect("level one node should exist");

        assert_ne!(level_one_label, "时间");
        assert!(
            level_one_label.contains("1982年") && level_one_label.contains("出生"),
            "model table-header labels should be replaced with meaningful stage events"
        );
    }

    #[test]
    fn studio_mind_map_prompt_includes_user_direction_and_completeness_rules() {
        let sections = vec![StudioMindMapSection {
            title: "产品定位".to_owned(),
            first_index: 0,
            items: vec![StudioMindMapSectionItem {
                text: "产品需要说明定位、关键矛盾和落地路径。".to_owned(),
                citation: section_citation("产品定位", "20:40"),
            }],
        }];
        let section_refs = sections.iter().collect::<Vec<_>>();
        let command = studio_mind_map_chat_command(
            "围绕产品定位、关键矛盾和落地路径总结",
            &section_refs,
            72,
            Some("runtime.llm.rag_answer"),
        );

        let system_prompt = &command.messages[0].content;
        let user_prompt = &command.messages[1].content;
        assert!(system_prompt.contains("尽可能完整"));
        assert!(system_prompt.contains("专业思维导图"));
        assert!(system_prompt.contains("严格 JSON"));
        assert!(system_prompt.contains("禁止把序位、序号、时间、日期、事件、备注等表头作为 label"));
        assert!(user_prompt.contains("用户总结方向：围绕产品定位、关键矛盾和落地路径总结"));
        assert!(user_prompt.contains("[S1] 产品定位"));
        assert_eq!(command.max_tokens, Some(3200));
    }

    #[test]
    fn studio_service_expands_shallow_model_points_without_repeating_summaries() {
        let sections = vec![StudioMindMapSection {
            title: "AI战略".to_owned(),
            first_index: 0,
            items: vec![StudioMindMapSectionItem {
                text: "ONE、AI搜问、AI表格等能力走向主动推进。".to_owned(),
                citation: section_citation("AI战略", "20:30"),
            }],
        }];
        let section_refs = sections.iter().collect::<Vec<_>>();
        let content = build_model_mind_map_content(
            "未命名的笔记本",
            r#"{
              "summary": "围绕 AI 战略提炼主线",
              "branches": [
                {
                  "label": "AI战略",
                  "summary": "产品线走向主动推进",
                  "citationIndex": 1,
                  "children": [
                    {
                      "label": "主动服务",
                      "summary": "ONE、AI搜问、AI表格等能力从响应走向主动推进",
                      "citationIndex": 1,
                      "children": []
                    }
                  ]
                }
              ]
            }"#,
            &section_refs,
            6,
        )
        .expect("shallow model JSON should build mind map content");

        let nodes = content["nodes"]
            .as_array()
            .expect("nodes should be an array");
        assert!(
            nodes.iter().any(|node| node["level"] == 3),
            "backend should add a detail node when model returns only two levels"
        );
        let repeated_fact_count = nodes
            .iter()
            .flat_map(|node| [node["label"].as_str(), node["summary"].as_str()])
            .flatten()
            .filter(|value| value.contains("ONE、AI搜问、AI表格"))
            .count();
        assert_eq!(
            repeated_fact_count, 1,
            "auto-expanded detail text should not be repeated in parent or child summaries"
        );
    }

    #[test]
    fn studio_mind_map_contexts_spread_generic_topic_across_document() {
        let records = (0..300)
            .map(|index| {
                chunk_record(
                    &format!("章节 {}", index / 50 + 1),
                    index,
                    format!("第 {index} 段描述了产品、组织和阶段性变化，用于验证全文覆盖。"),
                )
            })
            .collect::<Vec<_>>();

        let contexts = studio_mind_map_contexts("未命名的笔记本", records, 18);
        let chunk_indices = contexts
            .iter()
            .filter_map(|context| context.citation.chunk_id.split(':').nth(1))
            .filter_map(|index| index.parse::<i32>().ok())
            .collect::<Vec<_>>();

        assert!(
            contexts.len() >= 288,
            "complete mind maps should use a larger retrieval budget"
        );
        assert!(
            chunk_indices.iter().any(|index| *index >= 250),
            "generic topic should include late document chunks instead of only the opening"
        );
        assert!(
            chunk_indices.iter().any(|index| *index <= 10),
            "generic topic should still keep opening context"
        );
    }

    #[test]
    fn studio_mind_map_generation_does_not_use_full_rag_answer_loop() {
        let source = include_str!("studio_service.rs");
        let forbidden_constructor = ["Knowledge", "Service::new"].concat();
        let forbidden_command = ["Rag", "Ask", "Command"].concat();

        assert!(
            !source.contains(&forbidden_constructor),
            "Studio mind map generation should not call the full RAG answer service"
        );
        assert!(
            !source.contains(&forbidden_command),
            "Studio mind map generation should not build a full RAG ask command"
        );
    }
}
