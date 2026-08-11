use std::time::Instant;

use axum::{body::Bytes, http::HeaderMap, Json};

use crate::{auth::ApiKey, error::AppError, models::ConvertResult};

pub async fn convert_raw(
    _auth: ApiKey,
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
        .and_then(|f| anydoc::Format::from_extension(f))
        .or_else(|| anydoc::Format::from_bytes(&body));

    let start = Instant::now();

    // Call anydoc directly to preserve ConvertError type for proper HTTP mapping
    let result = tokio::task::spawn_blocking(move || {
        let detected = anydoc::Format::from_bytes(&body);
        anydoc::to_markdown_bytes(&body, format_hint.or(detected))
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("task panicked: {e}")))?;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(markdown) => Ok(Json(ConvertResult {
            index: 0,
            filename,
            markdown: Some(markdown),
            error: None,
            elapsed_ms,
        })),
        Err(e) => Err(AppError::Convert(e)),
    }
}
