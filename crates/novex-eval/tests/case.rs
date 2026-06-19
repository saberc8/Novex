use novex_eval::*;
use serde_json::json;

#[test]
fn eval_runtime_expected_payload_defaults_fields_for_intent_and_tool_cases() {
    let intent_expected = serde_json::from_value::<EvalCaseExpected>(json!({
        "intent": "rag_question"
    }))
    .unwrap();
    let tool_expected = serde_json::from_value::<EvalCaseExpected>(json!({
        "toolCode": "feishu.message.send"
    }))
    .unwrap();

    assert_eq!(intent_expected.answer_contains, Vec::<String>::new());
    assert_eq!(intent_expected.citations, Vec::<String>::new());
    assert_eq!(intent_expected.intent.as_deref(), Some("rag_question"));
    assert_eq!(
        tool_expected.tool_code.as_deref(),
        Some("feishu.message.send")
    );
}
