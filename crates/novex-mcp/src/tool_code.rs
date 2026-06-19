pub fn mcp_tool_code(server_code: &str, tool_name: &str) -> String {
    format!(
        "mcp.{}.{}",
        normalize_mcp_code_segment(server_code),
        normalize_mcp_code_segment(tool_name)
    )
}

fn normalize_mcp_code_segment(value: &str) -> String {
    let normalized: String = value
        .trim()
        .chars()
        .map(|ch| match ch {
            ch if ch.is_ascii_alphanumeric() => ch.to_ascii_lowercase(),
            '.' | '_' => ch,
            '-' | '/' | ':' | ' ' => '_',
            _ => '_',
        })
        .collect();
    let collapsed = normalized
        .split('_')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if collapsed.is_empty() {
        "unknown".to_owned()
    } else {
        collapsed
    }
}
