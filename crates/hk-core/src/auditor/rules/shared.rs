pub(super) fn descriptive_line_mask(content: &str) -> Vec<bool> {
    let mut mask = Vec::new();
    let mut in_code_fence = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            mask.push(true);
        } else if in_code_fence || trimmed.starts_with('>') {
            mask.push(true);
        } else {
            mask.push(false);
        }
    }
    mask
}
