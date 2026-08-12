use serde::{Deserialize, Serialize};

/// Result of converting a single file (markdown + optional JSON structure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertResult {
    pub index: usize,
    pub filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<DocumentJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

/// Structured JSON output for a parsed document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentJson {
    /// Per-page content (PDFs and multi-page images)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<Vec<PageContent>>,
    /// Whether any OCR was performed on this document
    pub has_ocr: bool,
    /// Indices of pages that were OCR'd (0-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_page_indices: Option<Vec<usize>>,
    /// Detected format
    pub format: String,
    /// Total page count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<usize>,
    /// OCR confidence scores per page (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_confidence: Option<Vec<f64>>,
}

/// Content of a single page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    /// 0-based page index
    pub page: usize,
    /// How this page's content was obtained
    #[serde(rename = "type")]
    pub content_type: PageType,
    /// The extracted text content
    pub content: String,
    /// OCR confidence (0.0-1.0), only for OCR'd pages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageType {
    Text,
    Ocr,
    Mixed,
}

#[derive(Debug, Deserialize)]
pub struct ConvertQuery {
    pub format: Option<String>,
    /// Enable OCR for image-based PDF pages and standalone images
    #[serde(default = "default_true")]
    pub ocr: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    /// Whether OCR support is available (tesseract found on PATH)
    pub ocr_available: bool,
}
