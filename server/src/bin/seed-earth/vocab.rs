use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

use crate::http;

const MACRO: &str = "https://macrostrat.org/api/v2";

#[derive(Debug, Deserialize)]
struct MacroWrap<T> {
    success: MacroSuccess<T>,
}

#[derive(Debug, Deserialize)]
struct MacroSuccess<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct Interval {
    int_id: i32,
    name: String,
    abbrev: Option<String>,
    t_age: f64,
    b_age: f64,
    int_type: Option<String>,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Lith {
    lith_id: i32,
    name: String,
    #[serde(rename = "type")]
    lith_type: Option<String>,
    group: Option<String>,
    class: Option<String>,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LithAtt {
    lith_att_id: i32,
    name: String,
    #[serde(rename = "type")]
    attr_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Environ {
    environ_id: i32,
    name: String,
    #[serde(rename = "type")]
    env_type: Option<String>,
    class: Option<String>,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StratName {
    strat_name: String,
    rank: Option<String>,
    strat_name_id: i32,
    t_age: Option<serde_json::Value>,
    b_age: Option<serde_json::Value>,
    gp_id: Option<i32>,
    fm_id: Option<i32>,
    mbr_id: Option<i32>,
    sgp_id: Option<i32>,
}

pub async fn seed(pool: &PgPool) -> anyhow::Result<()> {
    seed_intervals(pool).await?;
    seed_lithologies(pool).await?;
    seed_lith_attributes(pool).await?;
    seed_environments(pool).await?;
    seed_strat_names(pool).await?;
    mark_retrieved(pool, "ics").await?;
    mark_retrieved(pool, "macrostrat").await?;
    Ok(())
}

async fn mark_retrieved(pool: &PgPool, code: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE earth.source SET retrieved_at = now() WHERE code = $1")
        .bind(code)
        .execute(pool)
        .await?;
    Ok(())
}

async fn seed_intervals(pool: &PgPool) -> anyhow::Result<()> {
    let url = format!("{MACRO}/defs/intervals?timescale=international%20intervals");
    let wrap: MacroWrap<Interval> = serde_json::from_value(http::get_json(&url).await?)
        .context("parse ICS/Macrostrat intervals")?;
    println!(
        "vocab: {} ICS/Macrostrat intervals",
        wrap.success.data.len()
    );

    let source_id: i16 = sqlx::query_scalar("SELECT id FROM earth.source WHERE code = 'ics'")
        .fetch_one(pool)
        .await?;

    for iv in &wrap.success.data {
        let rank = int_type_to_rank(iv.int_type.as_deref().unwrap_or("age"));
        let start = iv.b_age.max(iv.t_age);
        let end = iv.t_age.min(iv.b_age);
        sqlx::query(
            "INSERT INTO earth.time_unit
                (rank_id, name, abbrev, color_hex, start_ma, end_ma, ics_name, macrostrat_int_id)
             SELECT r.id, $1, $2, $3, $4, $5, $1, $6
             FROM earth.time_rank r
             WHERE r.name = $7
             ON CONFLICT (rank_id, name) DO UPDATE SET
                abbrev = EXCLUDED.abbrev,
                color_hex = EXCLUDED.color_hex,
                start_ma = EXCLUDED.start_ma,
                end_ma = EXCLUDED.end_ma,
                ics_name = EXCLUDED.ics_name,
                macrostrat_int_id = EXCLUDED.macrostrat_int_id",
        )
        .bind(&iv.name)
        .bind(iv.abbrev.as_deref())
        .bind(iv.color.as_deref())
        .bind(start)
        .bind(end)
        .bind(iv.int_id)
        .bind(rank)
        .execute(pool)
        .await
        .with_context(|| format!("insert interval {}", iv.name))?;

        sqlx::query(
            "INSERT INTO earth.source_record (source_id, external_id)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(source_id)
        .bind(iv.int_id.to_string())
        .execute(pool)
        .await?;
    }

    // Informal Hadean (ICS chart; Macrostrat eons start at the Archean).
    sqlx::query(
        "INSERT INTO earth.time_unit
            (rank_id, name, abbrev, color_hex, start_ma, end_ma, ics_name)
         SELECT r.id, 'Hadean', 'Hd', '#C62D3C', 4567, 4031, 'Hadean'
         FROM earth.time_rank r WHERE r.name = 'eon'
         ON CONFLICT (rank_id, name) DO NOTHING",
    )
    .execute(pool)
    .await?;

    link_time_parents(pool).await?;
    Ok(())
}

fn int_type_to_rank(t: &str) -> &'static str {
    match t.to_ascii_lowercase().as_str() {
        "eon" | "eonothem" => "eon",
        "era" | "erathem" => "era",
        "period" | "system" => "period",
        "subperiod" | "subsystem" => "subperiod",
        "epoch" | "series" => "epoch",
        "age" | "stage" | "subage" => "age",
        _ => "age",
    }
}

async fn link_time_parents(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE earth.time_unit child
         SET parent_id = (
            SELECT parent.id
            FROM earth.time_unit parent
            JOIN earth.time_rank cr ON cr.id = child.rank_id
            JOIN earth.time_rank pr ON pr.id = parent.rank_id
            WHERE pr.level < cr.level
              AND parent.start_ma >= child.start_ma - 0.001
              AND parent.end_ma <= child.end_ma + 0.001
            ORDER BY pr.level DESC, (parent.start_ma - parent.end_ma) ASC
            LIMIT 1
         )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_lithologies(pool: &PgPool) -> anyhow::Result<()> {
    let url = format!("{MACRO}/defs/lithologies?all");
    let wrap: MacroWrap<Lith> = serde_json::from_value(http::get_json(&url).await?)?;
    println!("vocab: {} lithologies", wrap.success.data.len());

    let source_id: i16 =
        sqlx::query_scalar("SELECT id FROM earth.source WHERE code = 'macrostrat'")
            .fetch_one(pool)
            .await?;

    for lith in &wrap.success.data {
        let class_name = lith.class.as_deref().unwrap_or("other");
        let class_name = match class_name {
            "sedimentary" | "igneous" | "metamorphic" => class_name,
            _ => "other",
        };
        sqlx::query(
            "INSERT INTO earth.lithology
                (class_id, name, lith_type, lith_group, color_hex, macrostrat_lith_id)
             SELECT c.id, $1, $2, $3, $4, $5
             FROM earth.rock_class c
             WHERE c.name = $6
             ON CONFLICT (macrostrat_lith_id) DO UPDATE SET
                name = EXCLUDED.name,
                lith_type = EXCLUDED.lith_type,
                lith_group = EXCLUDED.lith_group,
                color_hex = EXCLUDED.color_hex",
        )
        .bind(&lith.name)
        .bind(lith.lith_type.as_deref())
        .bind(empty_to_none(lith.group.as_deref()))
        .bind(lith.color.as_deref())
        .bind(lith.lith_id)
        .bind(class_name)
        .execute(pool)
        .await?;

        sqlx::query(
            "INSERT INTO earth.source_record (source_id, external_id)
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(source_id)
        .bind(format!("lith:{}", lith.lith_id))
        .execute(pool)
        .await?;
    }

    sqlx::query(
        "UPDATE earth.lithology child
         SET parent_id = (
            SELECT parent.id FROM earth.lithology parent
            WHERE parent.id <> child.id
              AND (
                (child.lith_group IS NOT NULL AND lower(parent.name) = lower(child.lith_group))
                OR (child.lith_group IS NULL AND child.lith_type IS NOT NULL
                    AND lower(parent.name) = lower(child.lith_type)
                    AND lower(parent.name) <> lower(child.name))
              )
            ORDER BY CASE WHEN child.lith_group IS NOT NULL
                          AND lower(parent.name) = lower(child.lith_group) THEN 0 ELSE 1 END
            LIMIT 1
         )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn seed_lith_attributes(pool: &PgPool) -> anyhow::Result<()> {
    let url = format!("{MACRO}/defs/lithology_attributes?all");
    let wrap: MacroWrap<LithAtt> = serde_json::from_value(http::get_json(&url).await?)?;
    println!("vocab: {} lithology attributes", wrap.success.data.len());
    for att in wrap.success.data {
        sqlx::query(
            "INSERT INTO earth.lithology_attribute (name, attr_type, macrostrat_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (name) DO UPDATE SET
                attr_type = EXCLUDED.attr_type,
                macrostrat_id = EXCLUDED.macrostrat_id",
        )
        .bind(att.name)
        .bind(att.attr_type)
        .bind(att.lith_att_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn seed_environments(pool: &PgPool) -> anyhow::Result<()> {
    let url = format!("{MACRO}/defs/environments?all");
    let wrap: MacroWrap<Environ> = serde_json::from_value(http::get_json(&url).await?)?;
    println!("vocab: {} environments", wrap.success.data.len());
    for env in &wrap.success.data {
        sqlx::query(
            "INSERT INTO earth.environment
                (name, env_type, env_class, color_hex, macrostrat_env_id)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (macrostrat_env_id) DO UPDATE SET
                name = EXCLUDED.name,
                env_type = EXCLUDED.env_type,
                env_class = EXCLUDED.env_class,
                color_hex = EXCLUDED.color_hex",
        )
        .bind(&env.name)
        .bind(env.env_type.as_deref())
        .bind(env.class.as_deref())
        .bind(env.color.as_deref())
        .bind(env.environ_id)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        "UPDATE earth.environment child
         SET parent_id = (
            SELECT parent.id FROM earth.environment parent
            WHERE parent.id <> child.id
              AND child.env_type IS NOT NULL
              AND lower(parent.name) = lower(child.env_type)
            LIMIT 1
         )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

fn empty_to_none(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

fn json_f64(v: Option<&serde_json::Value>) -> Option<f64> {
    match v? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn rank_code(rank: Option<&str>) -> &'static str {
    match rank.unwrap_or("").trim() {
        "Sgp" | "sgp" | "Supergroup" => "Sgp",
        "Gp" | "gp" | "Group" => "Gp",
        "Subgp" | "subgp" | "Subgroup" => "Subgp",
        "Fm" | "fm" | "Formation" => "Fm",
        "Mbr" | "mbr" | "Member" => "Mbr",
        "Bed" | "bed" => "Bed",
        _ => "Fm",
    }
}

async fn seed_strat_names(pool: &PgPool) -> anyhow::Result<()> {
    let url = format!("{MACRO}/defs/strat_names?all");
    let wrap: MacroWrap<StratName> = serde_json::from_value(http::get_json(&url).await?)?;
    println!("vocab: {} stratigraphic names", wrap.success.data.len());

    for (i, sn) in wrap.success.data.iter().enumerate() {
        let t_age = json_f64(sn.t_age.as_ref());
        let b_age = json_f64(sn.b_age.as_ref());
        sqlx::query(
            "INSERT INTO earth.strat_unit
                (rank_id, name, t_age_ma, b_age_ma, macrostrat_strat_id)
             SELECT r.id, $1, $2, $3, $4
             FROM earth.lithostrat_rank r
             WHERE r.code = $5
             ON CONFLICT (macrostrat_strat_id) DO UPDATE SET
                name = EXCLUDED.name,
                t_age_ma = EXCLUDED.t_age_ma,
                b_age_ma = EXCLUDED.b_age_ma",
        )
        .bind(&sn.strat_name)
        .bind(t_age)
        .bind(b_age)
        .bind(sn.strat_name_id)
        .bind(rank_code(sn.rank.as_deref()))
        .execute(pool)
        .await?;
        if i > 0 && i % 2000 == 0 {
            println!("vocab: inserted {i} strat names…");
        }
    }

    for sn in &wrap.success.data {
        let parent_ext = match rank_code(sn.rank.as_deref()) {
            "Bed" => nonzero(sn.mbr_id).or(nonzero(sn.fm_id)),
            "Mbr" => nonzero(sn.fm_id),
            "Fm" => nonzero(sn.gp_id),
            "Gp" => nonzero(sn.sgp_id),
            "Subgp" => nonzero(sn.gp_id).or(nonzero(sn.sgp_id)),
            _ => None,
        };
        let Some(parent_ext) = parent_ext else {
            continue;
        };
        sqlx::query(
            "UPDATE earth.strat_unit child
             SET parent_id = parent.id
             FROM earth.strat_unit parent
             WHERE child.macrostrat_strat_id = $1
               AND parent.macrostrat_strat_id = $2
               AND child.id <> parent.id",
        )
        .bind(sn.strat_name_id)
        .bind(parent_ext)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn nonzero(id: Option<i32>) -> Option<i32> {
    id.filter(|n| *n > 0)
}
