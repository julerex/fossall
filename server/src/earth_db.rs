//! Queries against the `earth` schema.

use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, Row};

pub const OCCURRENCE_CAP: i64 = 2000;

#[derive(Debug, Serialize)]
pub struct TimeUnit {
    pub id: i32,
    pub name: String,
    pub rank: String,
    pub color_hex: Option<String>,
    pub start_ma: f64,
    pub end_ma: f64,
}

#[derive(Debug, Serialize)]
pub struct TaxonHit {
    pub id: i64,
    pub scientific_name: String,
    pub rank: Option<String>,
    pub extant: Option<bool>,
    pub first_app_ma: Option<f64>,
    pub last_app_ma: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct OccurrencePoint {
    pub paleolat: f64,
    pub paleolng: f64,
    pub taxon_name: String,
    pub collection_name: Option<String>,
    pub max_ma: Option<f64>,
    pub min_ma: Option<f64>,
}

#[derive(Debug)]
pub struct ContinentRow {
    pub geom: Value,
    pub feature_code: String,
}

fn time_unit(row: &sqlx::postgres::PgRow) -> TimeUnit {
    TimeUnit {
        id: row.get("id"),
        name: row.get("name"),
        rank: row.get("rank"),
        color_hex: row.get("color_hex"),
        start_ma: row.get("start_ma"),
        end_ma: row.get("end_ma"),
    }
}

pub async fn timescale(pool: &PgPool) -> Result<Vec<TimeUnit>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT u.id, u.name, r.name AS rank, u.color_hex, u.start_ma, u.end_ma
         FROM earth.time_unit u
         JOIN earth.time_rank r ON r.id = u.rank_id
         ORDER BY r.level, u.start_ma DESC, u.end_ma",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(time_unit).collect())
}

pub async fn interval_at(pool: &PgPool, ma: f64) -> Result<Option<TimeUnit>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT u.id, u.name, r.name AS rank, u.color_hex, u.start_ma, u.end_ma
         FROM earth.time_unit u
         JOIN earth.time_rank r ON r.id = u.rank_id
         WHERE u.start_ma >= $1 AND u.end_ma <= $1
         ORDER BY (u.start_ma - u.end_ma) ASC, r.level DESC
         LIMIT 1",
    )
    .bind(ma)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(time_unit))
}

pub async fn continents(pool: &PgPool, ma: f64) -> Result<(f64, Vec<ContinentRow>), sqlx::Error> {
    let nearest: Option<f64> = sqlx::query_scalar(
        "SELECT r.time_ma
         FROM earth.reconstruction r
         ORDER BY abs(r.time_ma - $1)
         LIMIT 1",
    )
    .bind(ma)
    .fetch_optional(pool)
    .await?;
    let Some(time_ma) = nearest else {
        return Ok((ma, Vec::new()));
    };
    let rows = sqlx::query(
        "SELECT g.geom::text AS geom, t.code AS feature_code
         FROM earth.reconstruction r
         JOIN earth.reconstruction_geometry g ON g.reconstruction_id = r.id
         JOIN earth.land_feature_type t ON t.id = g.feature_type_id
         WHERE r.time_ma = $1",
    )
    .bind(time_ma)
    .fetch_all(pool)
    .await?;
    let mapped = rows
        .into_iter()
        .map(|row| ContinentRow {
            geom: serde_json::from_str(row.get::<String, _>("geom").as_str())
                .unwrap_or(Value::Null),
            feature_code: row.get("feature_code"),
        })
        .collect();
    Ok((time_ma, mapped))
}

pub async fn search_taxa(pool: &PgPool, q: &str, limit: i64) -> Result<Vec<TaxonHit>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT t.id, t.scientific_name, r.name AS rank, t.extant, t.first_app_ma, t.last_app_ma
         FROM earth.taxon t
         LEFT JOIN earth.taxon_rank r ON r.id = t.rank_id
         WHERE t.scientific_name ILIKE $1
         ORDER BY length(t.scientific_name), t.scientific_name
         LIMIT $2",
    )
    .bind(format!("{q}%"))
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| TaxonHit {
            id: row.get("id"),
            scientific_name: row.get("scientific_name"),
            rank: row.get("rank"),
            extant: row.get("extant"),
            first_app_ma: row.get("first_app_ma"),
            last_app_ma: row.get("last_app_ma"),
        })
        .collect())
}

pub async fn occurrences(
    pool: &PgPool,
    ma: f64,
    taxon_id: Option<i64>,
    limit: i64,
) -> Result<Vec<OccurrencePoint>, sqlx::Error> {
    let limit = limit.clamp(1, OCCURRENCE_CAP);
    let rows = if let Some(taxon_id) = taxon_id {
        sqlx::query(
            "WITH RECURSIVE kids AS (
                SELECT id FROM earth.taxon WHERE id = $2
                UNION ALL
                SELECT t.id FROM earth.taxon t
                JOIN kids ON t.parent_id = kids.id
             )
             SELECT c.paleolat, c.paleolng, t.scientific_name AS taxon_name,
                    c.name AS collection_name, c.max_ma, c.min_ma
             FROM earth.occurrence o
             JOIN kids k ON o.taxon_id = k.id
             JOIN earth.collection c ON c.id = o.collection_id
             JOIN earth.taxon t ON t.id = o.taxon_id
             WHERE c.paleolat IS NOT NULL AND c.paleolng IS NOT NULL
               AND c.max_ma >= $1 AND c.min_ma <= $1
             LIMIT $3",
        )
        .bind(ma)
        .bind(taxon_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT c.paleolat, c.paleolng, t.scientific_name AS taxon_name,
                    c.name AS collection_name, c.max_ma, c.min_ma
             FROM earth.occurrence o
             JOIN earth.collection c ON c.id = o.collection_id
             JOIN earth.taxon t ON t.id = o.taxon_id
             WHERE c.paleolat IS NOT NULL AND c.paleolng IS NOT NULL
               AND c.max_ma >= $1 AND c.min_ma <= $1
             ORDER BY o.id
             LIMIT $2",
        )
        .bind(ma)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };
    Ok(rows
        .iter()
        .map(|row| OccurrencePoint {
            paleolat: row.get("paleolat"),
            paleolng: row.get("paleolng"),
            taxon_name: row.get("taxon_name"),
            collection_name: row.get("collection_name"),
            max_ma: row.get("max_ma"),
            min_ma: row.get("min_ma"),
        })
        .collect())
}
