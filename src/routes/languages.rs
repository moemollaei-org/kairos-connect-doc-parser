use axum::{Extension, Json};
use serde::Serialize;

use crate::config::Config;

#[derive(Serialize)]
pub struct LanguagesResponse {
    /// Tesseract codes installed in this image, e.g. `["ara", "eng", "nld"]`.
    pub languages: Vec<String>,
    pub count: usize,
    /// Used when a request does not specify one.
    pub default: String,
}

/// GET /languages — what this deployment can actually read.
///
/// Callers need this to choose a `lang` value, and to tell "wrong code" from
/// "pack not installed in this image" without guessing.
pub async fn languages(Extension(config): Extension<Config>) -> Json<LanguagesResponse> {
    let installed = crate::languages::installed(&config).await;
    Json(LanguagesResponse {
        languages: installed.iter().cloned().collect(),
        count: installed.len(),
        default: config.ocr_languages.clone(),
    })
}
