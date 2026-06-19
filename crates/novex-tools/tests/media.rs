use novex_tools::*;

#[test]
fn media_image_generation_request_builds_provider_payload() {
    let request = MediaImageGenerationRequest::new("Create a training poster")
        .with_size("1024x1024")
        .with_count(2);

    assert_eq!(request.prompt, "Create a training poster");
    assert_eq!(
        request.to_provider_payload(),
        serde_json::json!({
            "prompt": "Create a training poster",
            "size": "1024x1024",
            "n": 2
        })
    );
}

#[test]
fn parse_media_image_generation_response_extracts_common_url_shapes() {
    let response = serde_json::json!({
        "id": "img-1",
        "data": [{
            "url": "https://cdn.example.com/img-1.png"
        }]
    });

    let result = parse_media_image_generation_response(&response)
        .expect("media image response should expose an asset url");

    assert_eq!(result.asset_url, "https://cdn.example.com/img-1.png");
    assert_eq!(result.provider_asset_id.as_deref(), Some("img-1"));
}
