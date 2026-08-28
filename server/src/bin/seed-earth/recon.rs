use anyhow::Context;
use serde_json::Value;
use sqlx::PgPool;

use crate::http;
use crate::simplify;

const GWS: &str = "https://gws.gplates.org/reconstruct";
const MODEL: &str = "CAO2024";
const MAX_MA: i32 = 1800;

pub async fn seed(pool: &PgPool) -> anyhow::Result<()> {
    let stride = http::env_f64("EARTH_RECON_STRIDE", 10.0).max(5.0);
    let epsilon = http::env_f64("EARTH_RECON_EPSILON", 0.35);
    let min_area = http::env_f64("EARTH_RECON_MIN_AREA", 0.5);
    let max_ma = http::env_f64("EARTH_RECON_MAX_MA", MAX_MA as f64);

    let model_id: i16 =
        sqlx::query_scalar("SELECT id FROM earth.plate_model WHERE code = 'CAO2024'")
            .fetch_one(pool)
            .await?;
    let feature_id: i16 =
        sqlx::query_scalar("SELECT id FROM earth.land_feature_type WHERE code = 'coastline'")
            .fetch_one(pool)
            .await?;

    let mut times = Vec::new();
    let mut t = 0.0;
    while t <= max_ma + 0.01 {
        times.push((t * 10.0).round() / 10.0);
        t += stride;
    }

    println!(
        "recon: {} snapshots, stride {stride} Ma, epsilon {epsilon}",
        times.len()
    );

    for (i, time_ma) in times.iter().copied().enumerate() {
        match seed_one(pool, model_id, feature_id, time_ma, epsilon, min_area).await {
            Ok(n) => println!(
                "recon: {time_ma} Ma → {n} features ({}/{})",
                i + 1,
                times.len()
            ),
            Err(err) => eprintln!("recon: {time_ma} Ma skipped: {err:#}"),
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    sqlx::query("UPDATE earth.source SET retrieved_at = now() WHERE code = 'gplates'")
        .execute(pool)
        .await?;
    Ok(())
}

async fn seed_one(
    pool: &PgPool,
    model_id: i16,
    feature_id: i16,
    time_ma: f64,
    epsilon: f64,
    min_area: f64,
) -> anyhow::Result<u64> {
    let url = format!("{GWS}/coastlines/?time={time_ma}&model={MODEL}");
    let body = http::get_json(&url)
        .await
        .with_context(|| format!("GWS coastlines {time_ma}"))?;
    let features = body
        .get("features")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let time_unit_id: Option<i32> = sqlx::query_scalar(
        "SELECT id FROM earth.time_unit
         WHERE start_ma >= $1 AND end_ma <= $1
         ORDER BY (start_ma - end_ma) ASC, rank_id DESC
         LIMIT 1",
    )
    .bind(time_ma)
    .fetch_optional(pool)
    .await?;

    let recon_id: i32 = sqlx::query_scalar(
        "INSERT INTO earth.reconstruction (plate_model_id, time_ma, time_unit_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (plate_model_id, time_ma) DO UPDATE SET time_unit_id = EXCLUDED.time_unit_id
         RETURNING id",
    )
    .bind(model_id)
    .bind(time_ma)
    .bind(time_unit_id)
    .fetch_one(pool)
    .await?;

    sqlx::query("DELETE FROM earth.reconstruction_geometry WHERE reconstruction_id = $1")
        .bind(recon_id)
        .execute(pool)
        .await?;

    let mut kept = 0u64;
    for feat in features {
        let Some(geom) = feat.get("geometry") else {
            continue;
        };
        let Some(simple) = simplify::simplify_geometry(geom, epsilon, min_area) else {
            continue;
        };
        let bbox = simplify::geometry_bbox(&simple);
        let (west, south, east, north) = bbox.unwrap_or((0.0, 0.0, 0.0, 0.0));
        sqlx::query(
            "INSERT INTO earth.reconstruction_geometry
                (reconstruction_id, feature_type_id, geom, bbox_west, bbox_south, bbox_east, bbox_north)
             VALUES ($1, $2, $3::jsonb, $4, $5, $6, $7)",
        )
        .bind(recon_id)
        .bind(feature_id)
        .bind(simple.to_string())
        .bind(west)
        .bind(south)
        .bind(east)
        .bind(north)
        .execute(pool)
        .await?;
        kept += 1;
    }
    Ok(kept)
}
