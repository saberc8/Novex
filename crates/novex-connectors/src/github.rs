use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCodeSearchRequest {
    pub repository: String,
    pub query: String,
    pub path: Option<String>,
    pub limit: usize,
}

impl GitHubCodeSearchRequest {
    pub fn new(repository: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            repository: repository.into().trim().to_owned(),
            query: query.into().trim().to_owned(),
            path: None,
            limit: 10,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        let path = path.into().trim().trim_start_matches('/').to_owned();
        if !path.is_empty() {
            self.path = Some(path);
        }
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.clamp(1, 100);
        self
    }

    pub fn rest_path(&self) -> String {
        "/search/code".to_owned()
    }

    pub fn query_pairs(&self) -> Vec<(String, String)> {
        let mut query = format!("{} repo:{}", self.query, self.repository);
        if let Some(path) = self.path.as_deref() {
            query.push_str(" path:");
            query.push_str(path);
        }
        vec![
            ("q".to_owned(), query),
            ("per_page".to_owned(), self.limit.to_string()),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubFileReadRequest {
    pub repository: String,
    pub path: String,
    pub reference: Option<String>,
}

impl GitHubFileReadRequest {
    pub fn new(repository: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            repository: repository.into().trim().to_owned(),
            path: normalize_github_path(path.into()),
            reference: None,
        }
    }

    pub fn with_ref(mut self, reference: impl Into<String>) -> Self {
        let reference = reference.into().trim().to_owned();
        if !reference.is_empty() {
            self.reference = Some(reference);
        }
        self
    }

    pub fn rest_path(&self) -> String {
        format!("/repos/{}/contents/{}", self.repository, self.path)
    }

    pub fn query_pairs(&self) -> Vec<(String, String)> {
        self.reference
            .as_ref()
            .map(|reference| vec![("ref".to_owned(), reference.clone())])
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCodeSearchItem {
    pub repository: String,
    pub path: String,
    pub name: Option<String>,
    pub html_url: Option<String>,
    pub score: Option<f32>,
}

pub fn parse_github_code_search_response(value: &Value) -> Vec<GitHubCodeSearchItem> {
    value
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(github_code_search_item_from_value)
        .collect()
}

fn normalize_github_path(path: String) -> String {
    path.trim()
        .trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .collect::<Vec<_>>()
        .join("/")
}

fn github_code_search_item_from_value(value: &Value) -> Option<GitHubCodeSearchItem> {
    let repository = value
        .get("repository")?
        .get("full_name")?
        .as_str()?
        .trim()
        .to_owned();
    let path = value.get("path")?.as_str()?.trim().to_owned();
    if repository.is_empty() || path.is_empty() {
        return None;
    }
    Some(GitHubCodeSearchItem {
        repository,
        path,
        name: value
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        html_url: value
            .get("html_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        score: value.get("score").and_then(json_f32),
    })
}

fn json_f32(value: &Value) -> Option<f32> {
    if let Some(value) = value.as_f64() {
        return Some(value as f32);
    }
    value.as_str()?.parse::<f32>().ok()
}
