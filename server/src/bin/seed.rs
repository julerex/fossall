//! Insert `data/five_letter_words.txt` into `words.five_letter_words`.

use std::path::PathBuf;

use anyhow::{bail, Context};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required to seed")?;
    let path = word_list_path()?;
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let words: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| line.len() == 5 && line.chars().all(|c| c.is_ascii_lowercase()))
        .map(str::to_string)
        .collect();
    if words.is_empty() {
        bail!("{} contained no five-letter words", path.display());
    }

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .context("connect")?;

    let result = sqlx::query(
        "INSERT INTO words.five_letter_words (word)
         SELECT UNNEST($1::text[])
         ON CONFLICT DO NOTHING",
    )
    .bind(&words)
    .execute(&pool)
    .await
    .context("insert")?;

    println!(
        "seeded {} words from {} ({} new rows)",
        words.len(),
        path.display(),
        result.rows_affected()
    );
    Ok(())
}

fn word_list_path() -> anyhow::Result<PathBuf> {
    let candidates = [
        PathBuf::from("data/five_letter_words.txt"),
        PathBuf::from("../data/five_letter_words.txt"),
    ];
    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }
    bail!("could not find data/five_letter_words.txt (run from repo root or server/)");
}
