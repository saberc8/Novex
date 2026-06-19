use crate::media::MediaImageGenerationRequest;
use novex_connectors::{GitHubCodeSearchRequest, GitHubFileReadRequest};
use serde_json::Value;

pub fn feishu_message_text_from_tool_input(input: &Value) -> String {
    non_empty_json_string(input.get("message"))
        .or_else(|| non_empty_json_string(input.get("text")))
        .or_else(|| non_empty_json_string(input.get("input")))
        .unwrap_or_else(|| "Novex notification".to_owned())
}

pub fn media_image_request_from_tool_input(input: &Value) -> MediaImageGenerationRequest {
    let prompt = non_empty_json_string(input.get("prompt"))
        .or_else(|| non_empty_json_string(input.get("message")))
        .or_else(|| non_empty_json_string(input.get("input")))
        .or_else(|| non_empty_json_string(input.get("text")))
        .unwrap_or_else(|| "Novex generated image".to_owned());
    let mut request = MediaImageGenerationRequest::new(prompt);
    if let Some(size) = non_empty_json_string(input.get("size")) {
        request = request.with_size(size);
    }
    if let Some(count) = json_usize(input.get("n")).or_else(|| json_usize(input.get("count"))) {
        request = request.with_count(count);
    }
    request
}

pub fn github_search_request_from_tool_input(input: &Value) -> Option<GitHubCodeSearchRequest> {
    let input_text = non_empty_json_string(input.get("input"));
    let repository = github_repository_from_tool_input(input)?;
    let query = non_empty_json_string(input.get("query"))
        .or_else(|| non_empty_json_string(input.get("search")))
        .or_else(|| {
            input_text
                .as_deref()
                .and_then(|text| github_search_query_from_text(text, &repository))
        })
        .or(input_text)?;
    let mut request = GitHubCodeSearchRequest::new(repository, query);
    if let Some(path) = non_empty_json_string(input.get("path")).or_else(|| {
        non_empty_json_string(input.get("input"))
            .as_deref()
            .and_then(github_search_path_from_text)
    }) {
        request = request.with_path(path);
    }
    if let Some(limit) = json_usize(input.get("limit")).or_else(|| json_usize(input.get("perPage")))
    {
        request = request.with_limit(limit);
    }
    Some(request)
}

pub fn github_read_request_from_tool_input(input: &Value) -> Option<GitHubFileReadRequest> {
    let input_text = non_empty_json_string(input.get("input"));
    let repository = github_repository_from_tool_input(input)?;
    let path = non_empty_json_string(input.get("path"))
        .or_else(|| non_empty_json_string(input.get("filePath")))
        .or_else(|| {
            input_text
                .as_deref()
                .and_then(|text| github_read_path_from_text(text, &repository))
        })?;
    let mut request = GitHubFileReadRequest::new(repository, path);
    if let Some(reference) = non_empty_json_string(input.get("ref"))
        .or_else(|| non_empty_json_string(input.get("reference")))
        .or_else(|| non_empty_json_string(input.get("branch")))
        .or_else(|| input_text.as_deref().and_then(github_ref_from_text))
    {
        request = request.with_ref(reference);
    }
    Some(request)
}

fn github_repository_from_tool_input(input: &Value) -> Option<String> {
    non_empty_json_string(input.get("repository"))
        .or_else(|| non_empty_json_string(input.get("repo")))
        .or_else(|| {
            non_empty_json_string(input.get("input"))
                .as_deref()
                .and_then(github_repository_from_text)
        })
        .filter(|value| value.contains('/') && !value.contains(".."))
}

fn github_repository_from_text(text: &str) -> Option<String> {
    github_text_tokens(text)
        .into_iter()
        .find(|token| is_github_repository_token(token))
}

fn github_search_query_from_text(text: &str, repository: &str) -> Option<String> {
    let tokens = github_text_tokens(text);
    let repo_index = tokens.iter().position(|token| token == repository)?;
    let mut start = repo_index + 1;
    if tokens
        .get(start)
        .is_some_and(|token| token.eq_ignore_ascii_case("for"))
    {
        start += 1;
    }
    let mut end = tokens.len();
    for index in start..tokens.len() {
        if tokens[index].eq_ignore_ascii_case("under")
            || tokens[index].eq_ignore_ascii_case("path")
            || (tokens[index].eq_ignore_ascii_case("in")
                && tokens
                    .get(index + 1)
                    .is_some_and(|token| token.eq_ignore_ascii_case("path")))
        {
            end = index;
            break;
        }
    }

    let query = tokens[start..end]
        .iter()
        .filter(|token| !github_search_filler_token(token))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        None
    } else {
        Some(query)
    }
}

fn github_search_path_from_text(text: &str) -> Option<String> {
    let tokens = github_text_tokens(text);
    for (index, token) in tokens.iter().enumerate() {
        if token.eq_ignore_ascii_case("under") || token.eq_ignore_ascii_case("path") {
            return tokens.get(index + 1).cloned();
        }
        if token.eq_ignore_ascii_case("in")
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.eq_ignore_ascii_case("path"))
        {
            return tokens.get(index + 2).cloned();
        }
    }
    None
}

fn github_read_path_from_text(text: &str, repository: &str) -> Option<String> {
    let tokens = github_text_tokens(text);
    let repo_index = tokens.iter().position(|token| token == repository)?;
    for token in tokens.iter().skip(repo_index + 1) {
        if github_ref_keyword(token) {
            return None;
        }
        if token.eq_ignore_ascii_case("file") || token.eq_ignore_ascii_case("path") {
            continue;
        }
        return Some(token.clone());
    }
    None
}

fn github_ref_from_text(text: &str) -> Option<String> {
    let tokens = github_text_tokens(text);
    for (index, token) in tokens.iter().enumerate() {
        if github_ref_keyword(token) {
            return tokens.get(index + 1).cloned();
        }
    }
    None
}

fn github_text_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|token| {
            let token = token.trim_matches(|ch: char| {
                matches!(
                    ch,
                    ',' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
                )
            });
            if token.is_empty() {
                None
            } else {
                Some(token.to_owned())
            }
        })
        .collect()
}

fn is_github_repository_token(token: &str) -> bool {
    let Some((owner, repo)) = token.split_once('/') else {
        return false;
    };
    !owner.is_empty()
        && !repo.is_empty()
        && !owner.contains("..")
        && !repo.contains("..")
        && !owner.contains('/')
        && !repo.contains('/')
}

fn github_search_filler_token(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "search" | "github" | "repo" | "repository" | "code" | "for"
    )
}

fn github_ref_keyword(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "ref" | "reference" | "branch"
    )
}

fn json_usize(value: Option<&Value>) -> Option<usize> {
    let value = value?;
    if let Some(number) = value.as_u64() {
        return Some(number.min(usize::MAX as u64) as usize);
    }
    value.as_str()?.trim().parse::<usize>().ok()
}

fn non_empty_json_string(value: Option<&Value>) -> Option<String> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
