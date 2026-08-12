use std::sync::Arc;

use axum::{
    Extension,
    body::Body,
    extract::{Multipart, Query},
    http::StatusCode,
    response::Response,
};
use bytes::Bytes;
use futures::StreamExt;
use tokio::sync::Semaphore;
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    auth::ApiKey,
    config::Config,
    converter::{self, ConvertInput},
    error::AppError,
    models::{ConvertQuery, ConvertResult},
};

pub async fn convert(
    _auth: ApiKey,
    Extension(config): Extension<Config>,
    Query(query): Query<ConvertQuery>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    // --- Buffer all file parts ---
    let mut inputs: Vec<ConvertInput> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart error: {e}")))?
    {
        let filename = field.file_name().unwrap_or("unnamed").to_string();
        let bytes = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("read error: {e}")))?;

        if inputs.len() >= 50 {
            return Err(AppError::BadRequest("too many files (max 50)".into()));
        }
        if bytes.len() > 200 * 1024 * 1024 {
            return Err(AppError::BadRequest(format!(
                "'{filename}' exceeds 200 MB limit"
            )));
        }

        let format_hint = query
            .format
            .as_deref()
            .and_then(anydoc::Format::from_extension)
            .or_else(|| anydoc::Format::from_bytes(&bytes));

        inputs.push(ConvertInput {
            index: inputs.len(),
            filename,
            bytes: bytes.to_vec(),
            format_hint,
            ocr_enabled: query.ocr,
        });
    }

    if inputs.is_empty() {
        return Err(AppError::BadRequest("no files provided".into()));
    }

    let total = inputs.len();

    // --- Convert concurrently with Semaphore ---
    let semaphore = Arc::new(Semaphore::new(config.max_concurrent));
    let (tx, rx) = tokio::sync::mpsc::channel::<ConvertResult>(total);

    for input in inputs {
        let tx = tx.clone();
        let cfg = config.clone();
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("semaphore: {e}")))?;

        tokio::spawn(async move {
            let result = converter::convert_one(&cfg, input).await;
            let _ = tx.send(result).await;
            drop(permit);
        });
    }

    drop(tx); // channel closes when all senders gone

    // --- Stream results as NDJSON ---
    let stream = ReceiverStream::new(rx).map(|result| {
        let mut json = serde_json::to_string(&result).unwrap_or_default();
        json.push('\n');
        Ok::<Bytes, std::convert::Infallible>(Bytes::from(json))
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-ndjson")
        .body(Body::from_stream(stream))
        .unwrap())
}
