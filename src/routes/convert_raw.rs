use std::time::Instant;

use axum::{Extension, Json, body::Bytes, extract::Query, http::HeaderMap};

use crate::{
    auth::ApiKey,
    config::Config,
    converter::{self, ConvertInput},
    error::AppError,
    models::{ConvertQuery, ConvertResult},
};

pub async fn convert_raw(
    _auth: ApiKey,
    Extension(config): Extension<Config>,
    Query(query): Query<ConvertQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ConvertResult>, AppError> {
    let filename = headers
        .get("x-doc-filename")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unnamed")
        .to_string();

    let format_hint = headers
        .get("x-doc-format")
        .and_then(|v| v.to_str().ok())
        .and_then(anydoc::Format::from_extension)
        .or_else(|| anydoc::Format::from_bytes(&body));

    let input = ConvertInput {
        index: 0,
        filename: filename.clone(),
        bytes: body.to_vec(),
        format_hint,
        ocr_enabled: query.ocr,
    };

    let start = Instant::now();
    let result = converter::convert_one(&config, input).await;
    let mut result = result;
    result.elapsed_ms = start.elapsed().as_millis() as u64;

    if let Some(ref e) = result.error {
        // Map OCR/conversion errors appropriately
        if e.starts_with("OCR failed") || e.starts_with("OCR task") {
            return Err(AppError::Ocr(e.clone()));
        }
        // For anydoc conversion errors, use the Convert error type
        if e.contains("Document is") || e.contains("Unsupported") || e.contains("malformed") {
            return Err(AppError::BadRequest(e.clone()));
        }
    }

    Ok(Json(result))
}
