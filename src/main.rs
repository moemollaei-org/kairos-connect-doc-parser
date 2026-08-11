mod auth;
mod config;
mod converter;
mod error;
mod models;
mod routes;

use std::net::SocketAddr;

use axum::{middleware, routing::get, routing::post, Router};
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = config::Config::from_env();

    // Inject Config into every request's extensions (required by ApiKey extractor)
    let config_mw = middleware::from_fn({
        let cfg = config.clone();
        move |mut req: axum::http::Request<axum::body::Body>, next: middleware::Next| {
            req.extensions_mut().insert(cfg.clone());
            async move { next.run(req).await }
        }
    });

    let app = Router::new()
        .route("/health", get(routes::health::health))
        .route("/convert", post(routes::convert::convert))
        .route("/convert/raw", post(routes::convert_raw::convert_raw))
        .layer(config_mw)
        .layer(RequestBodyLimitLayer::new(config.body_limit_bytes))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
