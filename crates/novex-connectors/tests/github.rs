use novex_connectors::{
    parse_github_code_search_response, GitHubCodeSearchRequest, GitHubFileReadRequest,
};

#[test]
fn github_code_search_request_builds_rest_path_and_query() {
    let request = GitHubCodeSearchRequest::new("acme/app", "parser worker")
        .with_path("src")
        .with_limit(5);

    assert_eq!(request.repository, "acme/app");
    assert_eq!(request.rest_path(), "/search/code");
    assert_eq!(
        request.query_pairs(),
        vec![
            (
                "q".to_owned(),
                "parser worker repo:acme/app path:src".to_owned()
            ),
            ("per_page".to_owned(), "5".to_owned())
        ]
    );
}

#[test]
fn github_file_read_request_builds_contents_path_with_ref() {
    let request = GitHubFileReadRequest::new("acme/app", "src/lib.rs").with_ref("main");

    assert_eq!(request.repository, "acme/app");
    assert_eq!(request.path, "src/lib.rs");
    assert_eq!(request.rest_path(), "/repos/acme/app/contents/src/lib.rs");
    assert_eq!(
        request.query_pairs(),
        vec![("ref".to_owned(), "main".to_owned())]
    );
}

#[test]
fn parse_github_code_search_response_maps_items() {
    let response = serde_json::json!({
        "items": [{
            "name": "lib.rs",
            "path": "src/lib.rs",
            "html_url": "https://github.com/acme/app/blob/main/src/lib.rs",
            "repository": {
                "full_name": "acme/app"
            },
            "score": 12.5
        }]
    });

    let items = parse_github_code_search_response(&response);

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].repository, "acme/app");
    assert_eq!(items[0].path, "src/lib.rs");
    assert_eq!(items[0].score, Some(12.5));
}
