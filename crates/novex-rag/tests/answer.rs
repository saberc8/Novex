use novex_rag::*;

#[test]
fn build_extractive_answer_returns_answer_and_citations() {
    let parsed = parse_plain_text(
        "doc-3",
        "Training starts on Monday. Mentors review progress weekly.",
    );
    let chunks = chunk_text(&parsed, 80, 0);
    let hits = keyword_retrieve("When does training start?", &chunks, 3);

    let answer = build_extractive_answer("When does training start?", &hits);

    assert!(answer.answer.contains("Training starts on Monday"));
    assert_eq!(answer.citations.len(), 1);
    assert_eq!(answer.trace.retrieval_hit_count, 1);
    assert_eq!(answer.trace.answer_strategy, "extractive");
}
