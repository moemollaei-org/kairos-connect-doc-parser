use axum::{Extension, Json};

use crate::{config::Config, models::HealthResponse, ocr};

pub async fn health(Extension(config): Extension<Config>) -> Json<HealthResponse> {
    // Check OCR availability (don't block — use a quick spawn check)
    let ocr_available = ocr::is_tesseract_available(&config).await;
    let languages = crate::languages::installed(&config).await.len();

    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        ocr_available,
        languages,
        build: option_env!("BUILD_SHA").unwrap_or("unknown"),
    })
}
