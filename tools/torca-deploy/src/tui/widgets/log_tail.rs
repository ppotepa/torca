pub fn tail(lines: &[String], limit: usize) -> Vec<String> {
    let start = lines.len().saturating_sub(limit);
    lines[start..].to_vec()
}
