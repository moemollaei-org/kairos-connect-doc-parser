use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::{config::Config, error::AppError};

pub struct ApiKey;

impl<S> FromRequestParts<S> for ApiKey
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let config = parts
            .extensions
            .get::<Config>()
            .expect("Config must be in request extensions");

        let provided = parts
            .headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim());

        match provided {
            Some(k) if k == config.api_key => Ok(Self),
            _ => Err(AppError::Unauthorized),
        }
    }
}
