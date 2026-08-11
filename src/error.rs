use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Unauthorized,
    BadRequest(String),
    Convert(anydoc::ConvertError),
    Internal(anyhow::Error),
}

impl AppError {
    fn status_and_message(&self) -> (StatusCode, String) {
        match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Invalid or missing API key".into()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::Convert(e) => {
                let msg = match e {
                    anydoc::ConvertError::Encrypted => {
                        "Document is encrypted or password-protected"
                    }
                    anydoc::ConvertError::Unsupported(_) => "Unsupported document format",
                    anydoc::ConvertError::Malformed { .. } => "Document is malformed or corrupt",
                    anydoc::ConvertError::ResourceLimit { .. } => "Document exceeds processing limits",
                    anydoc::ConvertError::MissingPart { .. } => {
                        "Required part of the document is missing"
                    }
                    anydoc::ConvertError::Io(_) => "Could not read the document",
                    _ => "Document conversion failed",
                };
                (StatusCode::UNPROCESSABLE_ENTITY, msg.into())
            }
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".into(),
            ),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = self.status_and_message();
        // Log internal errors
        if let Self::Internal(ref e) = self {
            tracing::error!("internal error: {e:?}");
        }
        (status, Json(json!({"error": message}))).into_response()
    }
}

impl From<anydoc::ConvertError> for AppError {
    fn from(e: anydoc::ConvertError) -> Self {
        Self::Convert(e)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e)
    }
}
