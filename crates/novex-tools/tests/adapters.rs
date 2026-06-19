use novex_tools::*;

#[test]
fn agent_tool_input_feishu_message_text_prefers_explicit_message() {
    assert_eq!(
        feishu_message_text_from_tool_input(&serde_json::json!({
            "message": "Complete training today",
            "input": "ignored"
        })),
        "Complete training today"
    );
    assert_eq!(
        feishu_message_text_from_tool_input(&serde_json::json!({
            "input": "send a Feishu reminder"
        })),
        "send a Feishu reminder"
    );
    assert_eq!(
        feishu_message_text_from_tool_input(&serde_json::json!({})),
        "Novex notification"
    );
}

#[test]
fn agent_tool_input_media_image_request_prefers_prompt_size_and_count() {
    let request = media_image_request_from_tool_input(&serde_json::json!({
        "prompt": "Create a course poster",
        "input": "ignored",
        "size": "1024x1024",
        "count": 2
    }));

    assert_eq!(request.prompt, "Create a course poster");
    assert_eq!(request.size.as_deref(), Some("1024x1024"));
    assert_eq!(request.count, 2);
    assert_eq!(
        request.to_provider_payload(),
        serde_json::json!({
            "prompt": "Create a course poster",
            "n": 2,
            "size": "1024x1024"
        })
    );
}

#[test]
fn agent_tool_input_github_search_accepts_structured_and_natural_language() {
    let structured = github_search_request_from_tool_input(&serde_json::json!({
        "repository": "acme/app",
        "query": "parser worker",
        "path": "src",
        "limit": 5
    }))
    .expect("github search input should be valid");

    assert_eq!(structured.repository, "acme/app");
    assert_eq!(structured.query, "parser worker");
    assert_eq!(structured.path.as_deref(), Some("src"));
    assert_eq!(structured.limit, 5);

    let natural_language = github_search_request_from_tool_input(&serde_json::json!({
        "input": "search GitHub repo acme/app for parser worker under src"
    }))
    .expect("github search natural-language input should be valid");

    assert_eq!(natural_language.repository, "acme/app");
    assert_eq!(natural_language.query, "parser worker");
    assert_eq!(natural_language.path.as_deref(), Some("src"));
}

#[test]
fn agent_tool_input_github_read_accepts_structured_and_natural_language() {
    let structured = github_read_request_from_tool_input(&serde_json::json!({
        "repository": "acme/app",
        "path": "src/lib.rs",
        "ref": "main"
    }))
    .expect("github read input should be valid");

    assert_eq!(structured.repository, "acme/app");
    assert_eq!(structured.path, "src/lib.rs");
    assert_eq!(structured.reference.as_deref(), Some("main"));

    let natural_language = github_read_request_from_tool_input(&serde_json::json!({
        "input": "read GitHub file acme/app src/lib.rs ref main"
    }))
    .expect("github read natural-language input should be valid");

    assert_eq!(natural_language.repository, "acme/app");
    assert_eq!(natural_language.path, "src/lib.rs");
    assert_eq!(natural_language.reference.as_deref(), Some("main"));
}
