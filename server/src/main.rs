//! Fossall web server — Axum + maud HTML, static HTMX/CSS/WASM assets.

mod auth;
mod db;
mod demo;
mod earth;
mod earth_db;
mod layout;
mod pages;
mod shop;
mod words;

use axum::{
    routing::{get, post},
    Router,
};
use std::path::PathBuf;
use tower_http::services::ServeDir;

use crate::auth::ShopSettings;

#[derive(Clone)]
pub struct AppState {
    db: Option<sqlx::PgPool>,
    earth: Option<sqlx::PgPool>,
    shop: ShopSettings,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db = db::connect().await?;
    if db.is_some() {
        tracing::info!("postgres pool ready");
    } else {
        tracing::info!("DATABASE_URL unset; /words and /shop will return 503");
    }

    let earth = db::connect_earth().await?;
    if earth.is_some() {
        tracing::info!("earth postgres pool ready");
    } else {
        tracing::info!("EARTH_DATABASE_URL unset; /api/earth will return 503");
    }

    let shop = ShopSettings::from_env()?;
    if shop.login_configured() {
        tracing::info!("shop X login configured");
    } else {
        tracing::info!("shop X login unset; catalog still loads, sign-in stays disabled");
    }

    let static_root = static_dir();
    tracing::info!(path = %static_root.display(), "static root");

    let app = Router::new()
        .route("/", get(pages::home))
        .route("/rv", get(pages::rv_essay))
        .route("/homeprices", get(pages::homeprices))
        .route("/words", get(words::words))
        .route("/earth", get(earth::page))
        .route("/demo", get(demo::page))
        .route("/demo/list", get(demo::list))
        .route("/demo/messages", post(demo::create))
        .route("/demo/messages/{id}", get(demo::open).delete(demo::archive))
        .route("/demo/messages/{id}/restore", post(demo::restore))
        .route("/shop", get(shop::catalog))
        .route("/shop/cart", get(shop::cart).post(shop::update_cart))
        .route("/shop/checkout", post(shop::checkout))
        .route("/shop/orders", get(shop::orders))
        .route("/shop/orders/{id}", get(shop::order))
        .route("/shop/{sku}", get(shop::product))
        .route("/auth/x", get(auth::start))
        .route("/auth/x/callback", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        .route("/api/earth/timescale", get(earth::timescale))
        .route("/api/earth/continents", get(earth::continents))
        .route("/api/earth/taxa", get(earth::taxa))
        .route("/api/earth/occurrences", get(earth::occurrences))
        .route("/health", get(pages::health))
        .nest_service("/static", ServeDir::new(&static_root))
        .nest_service("/wasm", ServeDir::new(static_root.join("wasm")))
        .with_state(AppState { db, earth, shop });

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
