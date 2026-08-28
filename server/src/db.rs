//! Postgres pools for the words list and the earth database.

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

/// Connect using `DATABASE_URL`. `Ok(None)` if the variable is unset so local
/// essay pages still run without the proxy.
pub async fn connect() -> anyhow::Result<Option<PgPool>> {
    connect_env("DATABASE_URL").await
}

/// Connect using `EARTH_DATABASE_URL`. Independent of the words database.
pub async fn connect_earth() -> anyhow::Result<Option<PgPool>> {
    connect_env("EARTH_DATABASE_URL").await
}

async fn connect_env(var: &str) -> anyhow::Result<Option<PgPool>> {
    let Ok(url) = std::env::var(var) else {
        return Ok(None);
    };
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .max_lifetime(Duration::from_secs(600))
        .idle_timeout(Duration::from_secs(300))
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await?;
    Ok(Some(pool))
}

pub async fn count_words(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM words.five_letter_words")
        .fetch_one(pool)
        .await
}

pub async fn words_with_prefix(pool: &PgPool, prefix: &str) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT word FROM words.five_letter_words
         WHERE word LIKE $1
         ORDER BY word",
    )
    .bind(like_prefix(prefix))
    .fetch_all(pool)
    .await
}

fn like_prefix(prefix: &str) -> String {
    format!("{prefix}%")
}
