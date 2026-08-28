//! Seed the `earth` database from ICS / Macrostrat / PBDB / GPlates (CC-BY).
//!
//! Usage (EARTH_DATABASE_URL required):
//!   cargo run -p fossall-server --bin seed-earth
//!   cargo run -p fossall-server --bin seed-earth -- vocab
//!   cargo run -p fossall-server --bin seed-earth -- pbdb
//!   cargo run -p fossall-server --bin seed-earth -- recon
//!
//! Optional env:
//!   EARTH_PBDB_BASE_NAME   (e.g. Dinosauria — skip all_records)
//!   EARTH_PBDB_MAX_TAXA / EARTH_PBDB_MAX_COLLS / EARTH_PBDB_MAX_OCCS
//!   EARTH_RECON_STRIDE     (default 10 Ma)
//!   EARTH_RECON_MAX_MA     (default 1800)

mod http;
mod pbdb;
mod recon;
mod simplify;
mod vocab;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("EARTH_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .context("EARTH_DATABASE_URL (or DATABASE_URL) is required to seed")?;

    let target = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .context("connect")?;

    match target.as_str() {
        "vocab" => vocab::seed(&pool).await?,
        "pbdb" => pbdb::seed(&pool).await?,
        "recon" => recon::seed(&pool).await?,
        "all" => {
            vocab::seed(&pool).await?;
            pbdb::seed(&pool).await?;
            recon::seed(&pool).await?;
        }
        other => anyhow::bail!("unknown seed target {other:?} (use vocab, pbdb, recon, or all)"),
    }

    println!("seed-earth {target} complete");
    Ok(())
}
