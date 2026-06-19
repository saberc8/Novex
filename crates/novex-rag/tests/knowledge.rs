use novex_rag::{DatasetStatus, DocumentParseStatus, ResourceVisibility, RetrievalMode};

#[test]
fn knowledge_metadata_defaults_match_m1_control_plane() {
    assert_eq!(DatasetStatus::default(), DatasetStatus::Draft);
    assert_eq!(ResourceVisibility::default(), ResourceVisibility::Private);
    assert_eq!(RetrievalMode::default(), RetrievalMode::Hybrid);
    assert_eq!(DocumentParseStatus::default(), DocumentParseStatus::Pending);
}
