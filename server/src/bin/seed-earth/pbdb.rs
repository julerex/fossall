use anyhow::Context;
use serde_json::Value;
use sqlx::PgPool;

use crate::http;

const PBDB: &str = "https://paleobiodb.org/data1.2";
const PAGE: i32 = 2000;

pub async fn seed(pool: &PgPool) -> anyhow::Result<()> {
    let filter = std::env::var("EARTH_PBDB_BASE_NAME").ok();
    let max_taxa = http::env_u32("EARTH_PBDB_MAX_TAXA");
    let max_colls = http::env_u32("EARTH_PBDB_MAX_COLLS");
    let max_occs = http::env_u32("EARTH_PBDB_MAX_OCCS");

    seed_taxa(pool, filter.as_deref(), max_taxa).await?;
    link_taxon_parents(pool).await?;
    seed_collections(pool, filter.as_deref(), max_colls).await?;
    seed_occurrences(pool, filter.as_deref(), max_occs).await?;
    mark_retrieved(pool).await?;
    Ok(())
}

async fn mark_retrieved(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query("UPDATE earth.source SET retrieved_at = now() WHERE code = 'pbdb'")
        .execute(pool)
        .await?;
    Ok(())
}

fn list_url(kind: &str, extra: &str, offset: i32, filter: Option<&str>) -> String {
    let mut url = format!("{PBDB}/{kind}/list.json?vocab=pbdb&limit={PAGE}&offset={offset}");
    if let Some(name) = filter {
        url.push_str("&base_name=");
        url.push_str(&urlencoding_lite(name));
    } else {
        url.push_str("&all_records");
    }
    if !extra.is_empty() {
        url.push('&');
        url.push_str(extra);
    }
    url
}

fn urlencoding_lite(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => c.to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}

fn as_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().map(|n| n as i64))
        .or_else(|| v.as_f64().map(|n| n as i64))
        .or_else(|| v.as_str()?.split(':').next_back()?.parse().ok())
}

fn as_f64(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse().ok())
}

fn as_str(v: &Value) -> Option<&str> {
    v.as_str()
}

async fn seed_taxa(pool: &PgPool, filter: Option<&str>, max: Option<u32>) -> anyhow::Result<()> {
    let status_accepted: i16 =
        sqlx::query_scalar("SELECT id FROM earth.nomenclatural_status WHERE code = 'accepted'")
            .fetch_one(pool)
            .await?;
    let mut offset = 0i32;
    let mut total = 0u32;
    loop {
        let url = list_url("taxa", "show=attr,parent,app", offset, filter);
        let body = http::get_json(&url).await.context("taxa list")?;
        let records = body
            .get("records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if records.is_empty() {
            break;
        }
        let n = records.len();
        for rec in &records {
            insert_taxon(pool, rec, status_accepted).await?;
            total += 1;
            if max.is_some_and(|m| total >= m) {
                println!("pbdb: taxa {total} (capped)");
                return Ok(());
            }
        }
        println!("pbdb: taxa {total}");
        if n < PAGE as usize {
            break;
        }
        offset += PAGE;
    }
    println!("pbdb: taxa done ({total})");
    Ok(())
}

async fn insert_taxon(pool: &PgPool, rec: &Value, status_id: i16) -> anyhow::Result<()> {
    let Some(taxon_no) = rec.get("taxon_no").and_then(as_i64) else {
        return Ok(());
    };
    let name = rec
        .get("taxon_name")
        .and_then(as_str)
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return Ok(());
    }
    let rank = rec
        .get("taxon_rank")
        .and_then(as_str)
        .or_else(|| rec.get("accepted_rank").and_then(as_str));
    let auth = rec.get("taxon_attr").and_then(as_str);
    let parent_no = rec.get("parent_no").and_then(as_i64);
    let extant = rec.get("is_extant").and_then(as_str).map(|s| s == "extant");
    let first = rec
        .get("firstapp_max_ma")
        .and_then(as_f64)
        .or_else(|| rec.get("firstapp_min_ma").and_then(as_f64));
    let last = rec
        .get("lastapp_min_ma")
        .and_then(as_f64)
        .or_else(|| rec.get("lastapp_max_ma").and_then(as_f64));
    let year = auth.and_then(parse_year);

    let rank_name = rank.unwrap_or("informal");
    let rank_id: i16 = sqlx::query_scalar(
        "SELECT COALESCE(
            (SELECT id FROM earth.taxon_rank WHERE name = $1),
            (SELECT id FROM earth.taxon_rank WHERE name = 'informal')
         )",
    )
    .bind(rank_name)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO earth.taxon
            (rank_id, scientific_name, authorship, named_year, status_id, extant,
             pbdb_taxon_no, pbdb_parent_no, first_app_ma, last_app_ma)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (pbdb_taxon_no) DO UPDATE SET
            rank_id = EXCLUDED.rank_id,
            scientific_name = EXCLUDED.scientific_name,
            authorship = EXCLUDED.authorship,
            extant = EXCLUDED.extant,
            pbdb_parent_no = EXCLUDED.pbdb_parent_no,
            first_app_ma = EXCLUDED.first_app_ma,
            last_app_ma = EXCLUDED.last_app_ma",
    )
    .bind(rank_id)
    .bind(&name)
    .bind(auth)
    .bind(year)
    .bind(status_id)
    .bind(extant)
    .bind(taxon_no)
    .bind(parent_no)
    .bind(first)
    .bind(last)
    .execute(pool)
    .await?;
    Ok(())
}

fn parse_year(attr: &str) -> Option<i32> {
    attr.split_whitespace()
        .rev()
        .find_map(|tok| tok.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok())
}

async fn link_taxon_parents(pool: &PgPool) -> anyhow::Result<()> {
    let n = sqlx::query(
        "UPDATE earth.taxon child
         SET parent_id = parent.id
         FROM earth.taxon parent
         WHERE child.pbdb_parent_no IS NOT NULL
           AND parent.pbdb_taxon_no = child.pbdb_parent_no
           AND child.id <> parent.id",
    )
    .execute(pool)
    .await?
    .rows_affected();
    println!("pbdb: linked {n} taxon parents");

    let source_id: i16 = sqlx::query_scalar("SELECT id FROM earth.source WHERE code = 'pbdb'")
        .fetch_one(pool)
        .await?;
    sqlx::query(
        "INSERT INTO earth.taxon_opinion (child_id, parent_id, status_id, source_id)
         SELECT child.id, child.parent_id, child.status_id, $1
         FROM earth.taxon child
         WHERE child.parent_id IS NOT NULL
         ON CONFLICT DO NOTHING",
    )
    .bind(source_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_collections(
    pool: &PgPool,
    filter: Option<&str>,
    max: Option<u32>,
) -> anyhow::Result<()> {
    let mut offset = 0i32;
    let mut total = 0u32;
    loop {
        let url = list_url(
            "colls",
            "show=loc,paleoloc,stratext,lith,env,time",
            offset,
            filter,
        );
        let body = http::get_json(&url).await.context("colls list")?;
        let records = body
            .get("records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if records.is_empty() {
            break;
        }
        let n = records.len();
        for rec in &records {
            insert_collection(pool, rec).await?;
            total += 1;
            if max.is_some_and(|m| total >= m) {
                println!("pbdb: collections {total} (capped)");
                return Ok(());
            }
        }
        println!("pbdb: collections {total}");
        if n < PAGE as usize {
            break;
        }
        offset += PAGE;
    }
    println!("pbdb: collections done ({total})");
    Ok(())
}

async fn insert_collection(pool: &PgPool, rec: &Value) -> anyhow::Result<()> {
    let Some(no) = rec
        .get("collection_no")
        .and_then(as_i64)
        .or_else(|| rec.get("oid").and_then(as_i64))
    else {
        return Ok(());
    };
    let name = rec
        .get("collection_name")
        .and_then(as_str)
        .or_else(|| rec.get("nam").and_then(as_str));
    let lat = rec.get("lat").and_then(as_f64);
    let lng = rec.get("lng").and_then(as_f64);
    let paleolat = rec
        .get("paleolat")
        .and_then(as_f64)
        .or_else(|| rec.get("pla").and_then(as_f64));
    let paleolng = rec
        .get("paleolng")
        .and_then(as_f64)
        .or_else(|| rec.get("pln").and_then(as_f64));
    let cc = rec
        .get("cc")
        .and_then(as_str)
        .or_else(|| rec.get("cc2").and_then(as_str));
    let max_ma = rec
        .get("max_ma")
        .and_then(as_f64)
        .or_else(|| rec.get("eag").and_then(as_f64));
    let min_ma = rec
        .get("min_ma")
        .and_then(as_f64)
        .or_else(|| rec.get("lag").and_then(as_f64));
    let lith = rec
        .get("lithology1")
        .and_then(as_str)
        .or_else(|| rec.get("lt1").and_then(as_str))
        .map(unquote);
    let env = rec
        .get("environment")
        .and_then(as_str)
        .or_else(|| rec.get("env").and_then(as_str))
        .map(unquote);
    let formation = rec
        .get("formation")
        .and_then(as_str)
        .or_else(|| rec.get("sfm").and_then(as_str));
    let interval = rec
        .get("early_interval")
        .and_then(as_str)
        .or_else(|| rec.get("oei").and_then(as_str));

    sqlx::query(
        "INSERT INTO earth.collection
            (pbdb_collection_no, name, lat, lng, paleolat, paleolng, country_code,
             lithology_id, environment_id, time_unit_id, strat_unit_id, max_ma, min_ma)
         VALUES (
            $1, $2, $3, $4, $5, $6, $7,
            (SELECT id FROM earth.lithology WHERE lower(name) = lower($8) LIMIT 1),
            (SELECT id FROM earth.environment WHERE lower(name) = lower($9) LIMIT 1),
            (SELECT id FROM earth.time_unit WHERE lower(name) = lower($10) LIMIT 1),
            (SELECT id FROM earth.strat_unit WHERE lower(name) = lower($11) LIMIT 1),
            $12, $13
         )
         ON CONFLICT (pbdb_collection_no) DO UPDATE SET
            name = EXCLUDED.name,
            lat = EXCLUDED.lat,
            lng = EXCLUDED.lng,
            paleolat = EXCLUDED.paleolat,
            paleolng = EXCLUDED.paleolng,
            max_ma = EXCLUDED.max_ma,
            min_ma = EXCLUDED.min_ma",
    )
    .bind(no)
    .bind(name)
    .bind(lat)
    .bind(lng)
    .bind(paleolat)
    .bind(paleolng)
    .bind(cc)
    .bind(lith.as_deref())
    .bind(env.as_deref())
    .bind(interval)
    .bind(formation)
    .bind(max_ma)
    .bind(min_ma)
    .execute(pool)
    .await?;
    Ok(())
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').to_string()
}

async fn seed_occurrences(
    pool: &PgPool,
    filter: Option<&str>,
    max: Option<u32>,
) -> anyhow::Result<()> {
    let mut offset = 0i32;
    let mut total = 0u32;
    let mut skipped = 0u32;
    loop {
        let url = list_url("occs", "", offset, filter);
        let body = http::get_json(&url).await.context("occs list")?;
        let records = body
            .get("records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if records.is_empty() {
            break;
        }
        let n = records.len();
        for rec in &records {
            if insert_occurrence(pool, rec).await? {
                total += 1;
            } else {
                skipped += 1;
            }
            if max.is_some_and(|m| total >= m) {
                println!("pbdb: occurrences {total} (capped, skipped {skipped})");
                return Ok(());
            }
        }
        println!("pbdb: occurrences {total} (skipped {skipped})");
        if n < PAGE as usize {
            break;
        }
        offset += PAGE;
    }
    println!("pbdb: occurrences done ({total}, skipped {skipped})");
    Ok(())
}

async fn insert_occurrence(pool: &PgPool, rec: &Value) -> anyhow::Result<bool> {
    let Some(occ_no) = rec
        .get("occurrence_no")
        .and_then(as_i64)
        .or_else(|| rec.get("oid").and_then(as_i64))
    else {
        return Ok(false);
    };
    let Some(coll_no) = rec.get("collection_no").and_then(as_i64) else {
        return Ok(false);
    };
    let Some(taxon_no) = rec
        .get("taxon_no")
        .and_then(as_i64)
        .or_else(|| rec.get("accepted_no").and_then(as_i64))
    else {
        return Ok(false);
    };

    let result = sqlx::query(
        "INSERT INTO earth.occurrence (collection_id, taxon_id, pbdb_occurrence_no)
         SELECT c.id, t.id, $1
         FROM earth.collection c, earth.taxon t
         WHERE c.pbdb_collection_no = $2 AND t.pbdb_taxon_no = $3
         ON CONFLICT (pbdb_occurrence_no) DO NOTHING",
    )
    .bind(occ_no)
    .bind(coll_no)
    .bind(taxon_no)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
