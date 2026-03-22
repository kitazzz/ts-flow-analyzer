use crate::model::FunctionReport;

/// Stub: format function reports for LLM consumption
/// TODO: O7/O8 - Implement LLM summary formatting
#[allow(dead_code)]
pub fn format_for_llm(reports: &[FunctionReport]) -> String {
    serde_json::to_string_pretty(reports).unwrap_or_default()
}
