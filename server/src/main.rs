//! Fossall web server — Axum + maud HTML, static HTMX/CSS/WASM assets.

mod layout;
mod pages;

use axum::{routing::get, Router};
use std::path::PathBuf;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let static_root = static_dir();
    tracing::info!(path = %static_root.display(), "static root");

    let app = Router::new()
        .route("/", get(pages::home))
        .route("/rv", get(pages::rv_essay))
        .route("/homeprices", get(pages::homeprices))
        .route("/health", get(pages::health))
        .nest_service("/static", ServeDir::new(&static_root))
        .nest_service("/wasm", ServeDir::new(static_root.join("wasm")));

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("listening on http://0.0.0.0:{port}/");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Resolve `static/` for local dev (repo root or `server/`) and Docker (`/app/static`).
fn static_dir() -> PathBuf {
    let candidates = [
        PathBuf::from("static"),
        PathBuf::from("../static"),
        PathBuf::from("/app/static"),
    ];
    for path in &candidates {
        if path.join("css/style.css").exists() || path.join("htmx.min.js").exists() {
            return path.clone();
        }
    }
    candidates[0].clone()
}
