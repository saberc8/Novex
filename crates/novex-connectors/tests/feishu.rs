use novex_connectors::FeishuTextMessage;

#[test]
fn feishu_text_message_builds_custom_bot_payload() {
    let message = FeishuTextMessage::new("Training starts Monday");

    assert_eq!(message.text, "Training starts Monday");
    assert_eq!(
        message.to_webhook_payload(),
        serde_json::json!({
            "msg_type": "text",
            "content": {
                "text": "Training starts Monday"
            }
        })
    );
}
