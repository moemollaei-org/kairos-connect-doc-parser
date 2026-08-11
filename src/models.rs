use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ConvertResult {
    pub index: usize,
    pub filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConvertQuery {
    pub format: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}
