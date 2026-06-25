use std::{env, future::Future, pin::Pin, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;

use crate::shared::error::AppError;

pub const DEFAULT_RESEARCH_RADAR_LIMIT: u8 = 5;
const MAX_RESEARCH_RADAR_LIMIT: u8 = 10;
const RESEARCH_RADAR_TIMEOUT: Duration = Duration::from_secs(12);
const RESEARCH_RADAR_USER_AGENT: &str = "novex-research-radar-poc";

type SourceDispatchFuture =
    Pin<Box<dyn Future<Output = Result<Vec<ResearchRadarItem>, String>> + Send>>;
type SourceDispatcher =
    Arc<dyn Fn(ResearchRadarSource, String, u8) -> SourceDispatchFuture + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRadarScanCommand {
    pub topic: String,
    #[serde(default)]
    pub sources: Vec<ResearchRadarSource>,
    #[serde(default = "default_research_radar_ranking")]
    pub ranking: ResearchRadarRanking,
    pub limit_per_source: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRadarScanResp {
    pub topic: String,
    pub ranking: ResearchRadarRanking,
    pub status: ResearchRadarScanStatus,
    pub sources: Vec<ResearchRadarSourceResult>,
    pub items: Vec<ResearchRadarItem>,
    pub prompt_context: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRadarSourceResult {
    pub source: ResearchRadarSource,
    pub status: ResearchRadarSourceStatus,
    pub items: Vec<ResearchRadarItem>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRadarItem {
    pub id: String,
    pub source: ResearchRadarSource,
    pub kind: ResearchRadarItemKind,
    pub title: String,
    pub url: Option<String>,
    pub summary: Option<String>,
    pub authors: Vec<String>,
    pub organization: Option<String>,
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
    pub metrics: Vec<ResearchRadarMetric>,
    pub tags: Vec<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRadarMetric {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchRadarSource {
    Arxiv,
    Github,
    HuggingfaceModels,
    HuggingfaceDatasets,
    Paperswithcode,
    Leaderboards,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchRadarItemKind {
    Paper,
    Project,
    Model,
    Dataset,
    Benchmark,
    News,
    Community,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchRadarRanking {
    Balanced,
    Importance,
    Recency,
    Beginner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchRadarScanStatus {
    Succeeded,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchRadarSourceStatus {
    Succeeded,
    Failed,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedResearchRadarCommand {
    topic: String,
    sources: Vec<ResearchRadarSource>,
    ranking: ResearchRadarRanking,
    limit_per_source: u8,
}

#[derive(Clone)]
pub struct ResearchRadarService {
    dispatcher: SourceDispatcher,
}

impl ResearchRadarService {
    pub fn new() -> Self {
        Self::with_dispatcher(|source, topic, limit| async move {
            fetch_research_source(source, &topic, limit).await
        })
    }

    pub fn with_dispatcher<F, Fut>(dispatcher: F) -> Self
    where
        F: Fn(ResearchRadarSource, String, u8) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<ResearchRadarItem>, String>> + Send + 'static,
    {
        Self {
            dispatcher: Arc::new(move |source, topic, limit| {
                Box::pin(dispatcher(source, topic, limit))
            }),
        }
    }

    pub async fn scan(
        &self,
        command: ResearchRadarScanCommand,
    ) -> Result<ResearchRadarScanResp, AppError> {
        let command = normalize_scan_command(command)?;
        let mut source_results = Vec::new();
        let mut items = Vec::new();
        let mut warnings = Vec::new();

        for source in command.sources.iter().copied() {
            match (self.dispatcher)(source, command.topic.clone(), command.limit_per_source).await {
                Ok(mut source_items) => {
                    sort_items(&mut source_items, command.ranking);
                    items.extend(source_items.clone());
                    source_results.push(ResearchRadarSourceResult {
                        source,
                        status: ResearchRadarSourceStatus::Succeeded,
                        items: source_items,
                        warning: None,
                    });
                }
                Err(error) => {
                    let warning = format!("{}: {error}", source.label());
                    warnings.push(warning.clone());
                    source_results.push(ResearchRadarSourceResult {
                        source,
                        status: source_status_from_error(&error),
                        items: Vec::new(),
                        warning: Some(warning),
                    });
                }
            }
        }

        sort_items(&mut items, command.ranking);
        let status = scan_status(&source_results, &items);
        let prompt_context = build_prompt_context(&command.topic, command.ranking, &source_results);

        Ok(ResearchRadarScanResp {
            topic: command.topic,
            ranking: command.ranking,
            status,
            sources: source_results,
            items,
            prompt_context,
            warnings,
        })
    }
}

impl Default for ResearchRadarService {
    fn default() -> Self {
        Self::new()
    }
}

fn default_research_radar_ranking() -> ResearchRadarRanking {
    ResearchRadarRanking::Balanced
}

fn normalize_scan_command(
    command: ResearchRadarScanCommand,
) -> Result<NormalizedResearchRadarCommand, AppError> {
    let topic = command.topic.trim().to_owned();
    if topic.is_empty() {
        return Err(AppError::bad_request("研究主题不能为空"));
    }

    let sources = if command.sources.is_empty() {
        default_sources()
    } else {
        dedupe_sources(command.sources)
    };
    let limit_per_source = command
        .limit_per_source
        .unwrap_or(DEFAULT_RESEARCH_RADAR_LIMIT)
        .clamp(1, MAX_RESEARCH_RADAR_LIMIT);

    Ok(NormalizedResearchRadarCommand {
        topic,
        sources,
        ranking: command.ranking,
        limit_per_source,
    })
}

fn default_sources() -> Vec<ResearchRadarSource> {
    vec![
        ResearchRadarSource::Arxiv,
        ResearchRadarSource::Github,
        ResearchRadarSource::HuggingfaceModels,
        ResearchRadarSource::HuggingfaceDatasets,
        ResearchRadarSource::Paperswithcode,
        ResearchRadarSource::Leaderboards,
    ]
}

fn dedupe_sources(sources: Vec<ResearchRadarSource>) -> Vec<ResearchRadarSource> {
    let mut deduped = Vec::new();
    for source in sources {
        if !deduped.contains(&source) {
            deduped.push(source);
        }
    }
    deduped
}

async fn fetch_research_source(
    source: ResearchRadarSource,
    topic: &str,
    limit: u8,
) -> Result<Vec<ResearchRadarItem>, String> {
    match source {
        ResearchRadarSource::Arxiv => fetch_arxiv(topic, limit).await,
        ResearchRadarSource::Github => fetch_github_repositories(topic, limit).await,
        ResearchRadarSource::HuggingfaceModels => fetch_huggingface_models(topic, limit).await,
        ResearchRadarSource::HuggingfaceDatasets => fetch_huggingface_datasets(topic, limit).await,
        ResearchRadarSource::Paperswithcode => fetch_paperswithcode(topic, limit).await,
        ResearchRadarSource::Leaderboards => fetch_leaderboards(topic, limit).await,
    }
}

async fn fetch_arxiv(topic: &str, limit: u8) -> Result<Vec<ResearchRadarItem>, String> {
    let endpoint = env_string("NOVEX_RESEARCH_RADAR_ARXIV_ENDPOINT")
        .unwrap_or_else(|| "https://export.arxiv.org/api/query".to_owned());
    let mut url = Url::parse(&endpoint).map_err(|err| format!("arXiv endpoint invalid: {err}"))?;
    url.query_pairs_mut()
        .append_pair("search_query", &format!("all:{topic}"))
        .append_pair("start", "0")
        .append_pair("max_results", &limit.to_string())
        .append_pair("sortBy", "submittedDate")
        .append_pair("sortOrder", "descending");

    let body = http_get_text(url.as_str(), None).await?;
    parse_arxiv_atom_items(&body, limit as usize)
}

async fn fetch_github_repositories(
    topic: &str,
    limit: u8,
) -> Result<Vec<ResearchRadarItem>, String> {
    let base = env_string("GITHUB_API_BASE_URL")
        .or_else(|| env_string("NOVEX_GITHUB_API_BASE_URL"))
        .unwrap_or_else(|| "https://api.github.com".to_owned());
    let mut url = Url::parse(&format!(
        "{}/search/repositories",
        base.trim_end_matches('/')
    ))
    .map_err(|err| format!("GitHub endpoint invalid: {err}"))?;
    url.query_pairs_mut()
        .append_pair("q", topic)
        .append_pair("sort", "stars")
        .append_pair("order", "desc")
        .append_pair("per_page", &limit.to_string());

    let payload = http_get_json(
        url.as_str(),
        env_string("GITHUB_TOKEN").or_else(|| env_string("NOVEX_GITHUB_TOKEN")),
    )
    .await?;
    Ok(parse_github_repository_items(&payload, limit as usize))
}

async fn fetch_huggingface_models(
    topic: &str,
    limit: u8,
) -> Result<Vec<ResearchRadarItem>, String> {
    let payload = fetch_huggingface_json("/api/models", topic, limit).await?;
    Ok(parse_huggingface_model_items(&payload, limit as usize))
}

async fn fetch_huggingface_datasets(
    topic: &str,
    limit: u8,
) -> Result<Vec<ResearchRadarItem>, String> {
    let payload = fetch_huggingface_json("/api/datasets", topic, limit).await?;
    Ok(parse_huggingface_dataset_items(&payload, limit as usize))
}

async fn fetch_huggingface_json(path: &str, topic: &str, limit: u8) -> Result<Value, String> {
    let base = env_string("NOVEX_HUGGINGFACE_ENDPOINT")
        .or_else(|| env_string("HUGGINGFACE_ENDPOINT"))
        .unwrap_or_else(|| "https://huggingface.co".to_owned());
    let mut url = Url::parse(&format!("{}{}", base.trim_end_matches('/'), path))
        .map_err(|err| format!("Hugging Face endpoint invalid: {err}"))?;
    url.query_pairs_mut()
        .append_pair("search", topic)
        .append_pair("limit", &limit.to_string())
        .append_pair("sort", "likes")
        .append_pair("direction", "-1");

    http_get_json(
        url.as_str(),
        env_string("HUGGINGFACE_TOKEN")
            .or_else(|| env_string("HF_TOKEN"))
            .or_else(|| env_string("NOVEX_HUGGINGFACE_TOKEN")),
    )
    .await
}

async fn fetch_paperswithcode(topic: &str, limit: u8) -> Result<Vec<ResearchRadarItem>, String> {
    if let Some(endpoint) = env_string("NOVEX_RESEARCH_RADAR_PWC_ENDPOINT") {
        let payload = fetch_configured_json_endpoint(&endpoint, topic, limit).await?;
        return Ok(parse_generic_source_items(
            &payload,
            ResearchRadarSource::Paperswithcode,
            ResearchRadarItemKind::Paper,
            limit as usize,
        ));
    }

    Err("Papers With Code-compatible endpoint is not configured; paperswithcode.com currently redirects to Hugging Face Papers".to_owned())
}

async fn fetch_leaderboards(topic: &str, limit: u8) -> Result<Vec<ResearchRadarItem>, String> {
    let endpoints = env_string("NOVEX_RESEARCH_RADAR_LEADERBOARD_ENDPOINTS")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if endpoints.is_empty() {
        return Err("leaderboard endpoints are not configured".to_owned());
    }

    let mut items = Vec::new();
    let mut errors = Vec::new();
    for endpoint in endpoints {
        match fetch_configured_json_endpoint(&endpoint, topic, limit).await {
            Ok(payload) => items.extend(parse_generic_source_items(
                &payload,
                ResearchRadarSource::Leaderboards,
                ResearchRadarItemKind::Benchmark,
                limit as usize,
            )),
            Err(err) => errors.push(err),
        }
        if items.len() >= limit as usize {
            break;
        }
    }

    if items.is_empty() && !errors.is_empty() {
        Err(errors.join("; "))
    } else {
        items.truncate(limit as usize);
        Ok(items)
    }
}

async fn fetch_configured_json_endpoint(
    endpoint: &str,
    topic: &str,
    limit: u8,
) -> Result<Value, String> {
    let mut url =
        Url::parse(endpoint).map_err(|err| format!("configured endpoint invalid: {err}"))?;
    url.query_pairs_mut()
        .append_pair("q", topic)
        .append_pair("query", topic)
        .append_pair("limit", &limit.to_string());
    http_get_json(url.as_str(), None).await
}

async fn http_get_text(url: &str, token: Option<String>) -> Result<String, String> {
    let mut request = research_radar_http_client()?.get(url);
    if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|err| format!("source request failed: {err}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("source response read failed: {err}"))?;

    if !status.is_success() {
        return Err(format!("source returned HTTP {}", status.as_u16()));
    }
    Ok(body)
}

async fn http_get_json(url: &str, token: Option<String>) -> Result<Value, String> {
    let text = http_get_text(url, token).await?;
    serde_json::from_str::<Value>(&text)
        .map_err(|err| format!("source response JSON invalid: {err}"))
}

fn research_radar_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(RESEARCH_RADAR_TIMEOUT)
        .user_agent(RESEARCH_RADAR_USER_AGENT)
        .build()
        .map_err(|err| format!("research radar HTTP client init failed: {err}"))
}

fn parse_arxiv_atom_items(body: &str, limit: usize) -> Result<Vec<ResearchRadarItem>, String> {
    if !body.contains("<feed") && !body.contains("<entry") {
        return Err("arXiv response is not Atom XML".to_owned());
    }

    Ok(xml_blocks(body, "entry")
        .into_iter()
        .take(limit)
        .filter_map(|entry| {
            let id = xml_text(entry, "id").unwrap_or_default();
            let title = normalize_ws(&xml_text(entry, "title")?);
            if title.is_empty() {
                return None;
            }
            let summary = xml_text(entry, "summary").map(|value| normalize_ws(&value));
            let published_at = xml_text(entry, "published").map(|value| value.trim().to_owned());
            let updated_at = xml_text(entry, "updated").map(|value| value.trim().to_owned());
            let authors = xml_blocks(entry, "author")
                .into_iter()
                .filter_map(|author| xml_text(author, "name"))
                .map(|value| normalize_ws(&value))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            let url = xml_link_href(entry).or_else(|| non_empty(id.clone()));
            let item_id = non_empty(id)
                .or_else(|| url.clone())
                .unwrap_or_else(|| title.clone());

            Some(ResearchRadarItem {
                id: format!("arxiv:{item_id}"),
                source: ResearchRadarSource::Arxiv,
                kind: ResearchRadarItemKind::Paper,
                title,
                url,
                summary,
                authors,
                organization: None,
                published_at,
                updated_at,
                metrics: Vec::new(),
                tags: Vec::new(),
                metadata: json!({}),
            })
        })
        .collect())
}

fn parse_github_repository_items(payload: &Value, limit: usize) -> Vec<ResearchRadarItem> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(limit)
        .filter_map(|item| {
            let title = json_str(item, "full_name")?;
            let summary = json_str(item, "description");
            let url = json_str(item, "html_url");
            let updated_at = json_str(item, "updated_at");
            let language = json_str(item, "language");
            let mut metrics = Vec::new();
            push_metric(
                &mut metrics,
                "stars",
                json_f64(item.get("stargazers_count")),
            );
            push_metric(&mut metrics, "forks", json_f64(item.get("forks_count")));
            let tags = item
                .get("topics")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();

            Some(ResearchRadarItem {
                id: format!("github:{title}"),
                source: ResearchRadarSource::Github,
                kind: ResearchRadarItemKind::Project,
                title,
                url,
                summary,
                authors: Vec::new(),
                organization: None,
                published_at: None,
                updated_at,
                metrics,
                tags,
                metadata: json!({ "language": language }),
            })
        })
        .collect()
}

fn parse_huggingface_model_items(payload: &Value, limit: usize) -> Vec<ResearchRadarItem> {
    parse_huggingface_items(
        payload,
        limit,
        "modelId",
        ResearchRadarSource::HuggingfaceModels,
        ResearchRadarItemKind::Model,
        |id| format!("https://huggingface.co/{id}"),
    )
}

fn parse_huggingface_dataset_items(payload: &Value, limit: usize) -> Vec<ResearchRadarItem> {
    parse_huggingface_items(
        payload,
        limit,
        "id",
        ResearchRadarSource::HuggingfaceDatasets,
        ResearchRadarItemKind::Dataset,
        |id| format!("https://huggingface.co/datasets/{id}"),
    )
}

fn parse_huggingface_items<F>(
    payload: &Value,
    limit: usize,
    id_field: &str,
    source: ResearchRadarSource,
    kind: ResearchRadarItemKind,
    url_for_id: F,
) -> Vec<ResearchRadarItem>
where
    F: Fn(&str) -> String,
{
    payload
        .as_array()
        .into_iter()
        .flatten()
        .take(limit)
        .filter_map(|item| {
            let title = json_str(item, id_field).or_else(|| json_str(item, "id"))?;
            let mut metrics = Vec::new();
            push_metric(&mut metrics, "likes", json_f64(item.get("likes")));
            push_metric(&mut metrics, "downloads", json_f64(item.get("downloads")));
            let tags = item
                .get("tags")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            Some(ResearchRadarItem {
                id: format!("{}:{title}", source.code()),
                source,
                kind,
                title: title.clone(),
                url: Some(url_for_id(&title)),
                summary: json_str(item, "description"),
                authors: Vec::new(),
                organization: title.split('/').next().map(ToOwned::to_owned),
                published_at: None,
                updated_at: json_str(item, "lastModified").or_else(|| json_str(item, "updatedAt")),
                metrics,
                tags,
                metadata: json!({
                    "pipelineTag": json_str(item, "pipeline_tag"),
                    "private": item.get("private").and_then(Value::as_bool),
                }),
            })
        })
        .collect()
}

fn parse_generic_source_items(
    payload: &Value,
    source: ResearchRadarSource,
    kind: ResearchRadarItemKind,
    limit: usize,
) -> Vec<ResearchRadarItem> {
    generic_items_array(payload)
        .into_iter()
        .take(limit)
        .enumerate()
        .filter_map(|(index, item)| {
            let title = json_str(item, "title")
                .or_else(|| json_str(item, "name"))
                .or_else(|| json_str(item, "id"))?;
            let url = json_str(item, "url").or_else(|| json_str(item, "html_url"));
            let summary = json_str(item, "summary")
                .or_else(|| json_str(item, "description"))
                .or_else(|| json_str(item, "abstract"));
            let mut metrics = Vec::new();
            for label in ["score", "stars", "likes", "downloads", "accuracy", "metric"] {
                push_metric(&mut metrics, label, json_f64(item.get(label)));
            }
            Some(ResearchRadarItem {
                id: format!("{}:{index}:{title}", source.code()),
                source,
                kind,
                title,
                url,
                summary,
                authors: string_array(item.get("authors")),
                organization: json_str(item, "organization"),
                published_at: json_str(item, "publishedAt")
                    .or_else(|| json_str(item, "published_at")),
                updated_at: json_str(item, "updatedAt").or_else(|| json_str(item, "updated_at")),
                metrics,
                tags: string_array(item.get("tags")),
                metadata: item.clone(),
            })
        })
        .collect()
}

fn generic_items_array(payload: &Value) -> Vec<&Value> {
    if let Some(items) = payload.as_array() {
        return items.iter().collect();
    }
    for key in ["items", "results", "data", "leaderboards", "papers"] {
        if let Some(items) = payload.get(key).and_then(Value::as_array) {
            return items.iter().collect();
        }
    }
    Vec::new()
}

fn build_prompt_context(
    topic: &str,
    ranking: ResearchRadarRanking,
    source_results: &[ResearchRadarSourceResult],
) -> String {
    let mut lines = vec![
        "Research Radar Evidence".to_owned(),
        format!("Topic: {topic}"),
        format!("Ranking: {}", ranking.code()),
        String::new(),
    ];

    for result in source_results {
        if let Some(warning) = result.warning.as_deref() {
            lines.push(format!("[{}] Warning: {warning}", result.source.code()));
        }
        for item in result.items.iter().take(6) {
            lines.push(format!(
                "[{}] {}: {}",
                item.source.code(),
                item.kind.label(),
                item.title
            ));
            if !item.authors.is_empty() {
                lines.push(format!("Authors: {}", item.authors.join(", ")));
            }
            if let Some(date) = item.published_at.as_deref().or(item.updated_at.as_deref()) {
                lines.push(format!("Date: {date}"));
            }
            if !item.metrics.is_empty() {
                lines.push(format!(
                    "Metrics: {}",
                    item.metrics
                        .iter()
                        .map(|metric| format!("{}={}", metric.label, metric.value))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if let Some(url) = item.url.as_deref() {
                lines.push(format!("URL: {url}"));
            }
            if let Some(summary) = item.summary.as_deref() {
                lines.push(format!("Summary: {}", preview_text(summary, 260)));
            }
            lines.push(String::new());
        }
    }

    lines.join("\n")
}

fn scan_status(
    sources: &[ResearchRadarSourceResult],
    items: &[ResearchRadarItem],
) -> ResearchRadarScanStatus {
    let succeeded = sources
        .iter()
        .any(|source| source.status == ResearchRadarSourceStatus::Succeeded);
    let failed_or_degraded = sources
        .iter()
        .any(|source| source.status != ResearchRadarSourceStatus::Succeeded);

    if !succeeded || items.is_empty() {
        ResearchRadarScanStatus::Failed
    } else if failed_or_degraded {
        ResearchRadarScanStatus::Partial
    } else {
        ResearchRadarScanStatus::Succeeded
    }
}

fn source_status_from_error(error: &str) -> ResearchRadarSourceStatus {
    if error.contains("not configured") || error.contains("redirects") {
        ResearchRadarSourceStatus::Degraded
    } else {
        ResearchRadarSourceStatus::Failed
    }
}

fn sort_items(items: &mut [ResearchRadarItem], ranking: ResearchRadarRanking) {
    items.sort_by(|left, right| {
        let left_score = ranking_score(left, ranking);
        let right_score = ranking_score(right, ranking);
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.published_at.cmp(&left.published_at))
    });
}

fn ranking_score(item: &ResearchRadarItem, ranking: ResearchRadarRanking) -> f64 {
    match ranking {
        ResearchRadarRanking::Importance => metric_total(item),
        ResearchRadarRanking::Recency => date_score(item),
        ResearchRadarRanking::Beginner => {
            if item.summary.as_deref().unwrap_or("").len() > 80 {
                2.0
            } else {
                1.0
            }
        }
        ResearchRadarRanking::Balanced => metric_total(item).log10().max(0.0) + date_score(item),
    }
}

fn metric_total(item: &ResearchRadarItem) -> f64 {
    item.metrics
        .iter()
        .map(|metric| metric.value.max(0.0))
        .sum()
}

fn date_score(item: &ResearchRadarItem) -> f64 {
    item.updated_at
        .as_deref()
        .or(item.published_at.as_deref())
        .and_then(|value| value.get(0..4))
        .and_then(|year| year.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn xml_blocks<'a>(body: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut blocks = Vec::new();
    let mut rest = body;

    while let Some(start) = rest.find(&open) {
        let after_start = &rest[start..];
        let Some(start_end) = after_start.find('>') else {
            break;
        };
        let content_start = start + start_end + 1;
        let after_content_start = &rest[content_start..];
        let Some(end) = after_content_start.find(&close) else {
            break;
        };
        blocks.push(&rest[content_start..content_start + end]);
        rest = &rest[content_start + end + close.len()..];
    }

    blocks
}

fn xml_text(body: &str, tag: &str) -> Option<String> {
    xml_blocks(body, tag)
        .into_iter()
        .next()
        .map(decode_xml_entities)
}

fn xml_link_href(body: &str) -> Option<String> {
    let mut rest = body;
    while let Some(start) = rest.find("<link") {
        let link = &rest[start..];
        let end = link.find('>')?;
        let tag = &link[..end];
        if let Some(href) = attr_value(tag, "href").and_then(non_empty) {
            return Some(decode_xml_entities(&href));
        }
        rest = &link[end..];
    }
    None
}

fn attr_value(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = tag.find(&needle)? + needle.len();
    let tail = &tag[start..];
    let end = tail.find('"')?;
    Some(tail[..end].to_owned())
}

fn decode_xml_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
}

fn normalize_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn json_str(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn json_f64(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    value.as_str()?.parse::<f64>().ok()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn push_metric(metrics: &mut Vec<ResearchRadarMetric>, label: &str, value: Option<f64>) {
    if let Some(value) = value {
        metrics.push(ResearchRadarMetric {
            label: label.to_owned(),
            value,
        });
    }
}

fn preview_text(value: &str, limit: usize) -> String {
    let normalized = normalize_ws(value);
    if normalized.chars().count() <= limit {
        return normalized;
    }
    normalized.chars().take(limit).collect::<String>()
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

impl ResearchRadarSource {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Arxiv => "arxiv",
            Self::Github => "github",
            Self::HuggingfaceModels => "huggingface_models",
            Self::HuggingfaceDatasets => "huggingface_datasets",
            Self::Paperswithcode => "paperswithcode",
            Self::Leaderboards => "leaderboards",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Arxiv => "arXiv",
            Self::Github => "GitHub",
            Self::HuggingfaceModels => "Hugging Face models",
            Self::HuggingfaceDatasets => "Hugging Face datasets",
            Self::Paperswithcode => "Papers With Code",
            Self::Leaderboards => "Leaderboards",
        }
    }
}

impl ResearchRadarItemKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Paper => "Paper",
            Self::Project => "Project",
            Self::Model => "Model",
            Self::Dataset => "Dataset",
            Self::Benchmark => "Benchmark",
            Self::News => "News",
            Self::Community => "Community",
        }
    }
}

impl ResearchRadarRanking {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Importance => "importance",
            Self::Recency => "recency",
            Self::Beginner => "beginner",
        }
    }
}

#[cfg(test)]
fn test_item(title: &str, source: ResearchRadarSource) -> ResearchRadarItem {
    ResearchRadarItem {
        id: format!("{}:{title}", source.code()),
        source,
        kind: ResearchRadarItemKind::Paper,
        title: title.to_owned(),
        url: Some(format!("https://example.test/{title}")),
        summary: Some("test item".to_owned()),
        authors: Vec::new(),
        organization: None,
        published_at: Some("2026-01-01T00:00:00Z".to_owned()),
        updated_at: Some("2026-01-02T00:00:00Z".to_owned()),
        metrics: vec![ResearchRadarMetric {
            label: "score".to_owned(),
            value: 1.0,
        }],
        tags: Vec::new(),
        metadata: json!({}),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn research_radar_defaults_sources_and_limit() {
        let command = ResearchRadarScanCommand {
            topic: " agent workflow ".to_owned(),
            sources: vec![],
            ranking: ResearchRadarRanking::Balanced,
            limit_per_source: None,
        };

        let normalized = normalize_scan_command(command).unwrap();

        assert_eq!(normalized.topic, "agent workflow");
        assert_eq!(normalized.limit_per_source, 5);
        assert_eq!(
            normalized.sources,
            vec![
                ResearchRadarSource::Arxiv,
                ResearchRadarSource::Github,
                ResearchRadarSource::HuggingfaceModels,
                ResearchRadarSource::HuggingfaceDatasets,
                ResearchRadarSource::Paperswithcode,
                ResearchRadarSource::Leaderboards,
            ]
        );
    }

    #[test]
    fn parse_arxiv_atom_normalizes_paper_items() {
        let body = r#"
        <feed xmlns="http://www.w3.org/2005/Atom">
          <entry>
            <id>http://arxiv.org/abs/2401.12345v1</id>
            <updated>2024-01-03T00:00:00Z</updated>
            <published>2024-01-02T00:00:00Z</published>
            <title>Agent Workflow Planning</title>
            <summary>Workflow agents coordinate tools.</summary>
            <author><name>Ada Lovelace</name></author>
            <author><name>Grace Hopper</name></author>
            <link href="http://arxiv.org/abs/2401.12345v1" rel="alternate" type="text/html"/>
          </entry>
        </feed>
        "#;

        let items = parse_arxiv_atom_items(body, 5).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, ResearchRadarSource::Arxiv);
        assert_eq!(items[0].kind, ResearchRadarItemKind::Paper);
        assert_eq!(items[0].title, "Agent Workflow Planning");
        assert_eq!(items[0].authors, vec!["Ada Lovelace", "Grace Hopper"]);
        assert_eq!(
            items[0].published_at.as_deref(),
            Some("2024-01-02T00:00:00Z")
        );
        assert_eq!(
            items[0].url.as_deref(),
            Some("http://arxiv.org/abs/2401.12345v1")
        );
    }

    #[test]
    fn parse_github_repositories_normalizes_project_metrics() {
        let payload = json!({
            "items": [{
                "full_name": "acme/agent-workflow",
                "html_url": "https://github.com/acme/agent-workflow",
                "description": "Composable agent workflows",
                "stargazers_count": 1200,
                "forks_count": 88,
                "language": "Rust",
                "updated_at": "2026-06-01T00:00:00Z",
                "topics": ["agents", "workflow"]
            }]
        });

        let items = parse_github_repository_items(&payload, 5);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, ResearchRadarSource::Github);
        assert_eq!(items[0].kind, ResearchRadarItemKind::Project);
        assert_eq!(items[0].title, "acme/agent-workflow");
        assert_eq!(items[0].metrics[0].label, "stars");
        assert_eq!(items[0].metrics[0].value, 1200.0);
    }

    #[test]
    fn parse_huggingface_models_and_datasets_normalize_hub_payloads() {
        let models = json!([{
            "modelId": "acme/agent-model",
            "likes": 42,
            "downloads": 9001,
            "pipeline_tag": "text-generation",
            "lastModified": "2026-06-02T00:00:00.000Z",
            "tags": ["agents"]
        }]);
        let datasets = json!([{
            "id": "acme/agent-dataset",
            "likes": 12,
            "downloads": 300,
            "lastModified": "2026-06-03T00:00:00.000Z",
            "tags": ["benchmark"]
        }]);

        let model_items = parse_huggingface_model_items(&models, 5);
        let dataset_items = parse_huggingface_dataset_items(&datasets, 5);

        assert_eq!(model_items[0].kind, ResearchRadarItemKind::Model);
        assert_eq!(model_items[0].title, "acme/agent-model");
        assert_eq!(dataset_items[0].kind, ResearchRadarItemKind::Dataset);
        assert_eq!(dataset_items[0].title, "acme/agent-dataset");
    }

    #[tokio::test]
    async fn source_aggregation_returns_partial_when_one_provider_fails() {
        let service = ResearchRadarService::with_dispatcher(|source, _topic, _limit| async move {
            match source {
                ResearchRadarSource::Arxiv => {
                    Ok(vec![test_item("arxiv-paper", ResearchRadarSource::Arxiv)])
                }
                ResearchRadarSource::Github => Err("GitHub rate limited".to_owned()),
                _ => Ok(vec![]),
            }
        });

        let resp = service
            .scan(ResearchRadarScanCommand {
                topic: "agent workflow".to_owned(),
                sources: vec![ResearchRadarSource::Arxiv, ResearchRadarSource::Github],
                ranking: ResearchRadarRanking::Balanced,
                limit_per_source: Some(2),
            })
            .await
            .unwrap();

        assert_eq!(resp.status, ResearchRadarScanStatus::Partial);
        assert_eq!(resp.items.len(), 1);
        assert!(resp
            .warnings
            .iter()
            .any(|warning| warning.contains("GitHub rate limited")));
        assert!(resp.prompt_context.contains("[arxiv]"));
        assert!(!resp.prompt_context.contains("TOKEN"));
    }
}
