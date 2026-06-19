pub(crate) fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    value.to_lowercase().contains(&needle.to_lowercase())
}

pub(crate) fn round_score(score: f64) -> f64 {
    (score * 10_000.0).round() / 10_000.0
}
