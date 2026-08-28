//! `/earth` globe page and JSON API.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use maud::{html, Markup, PreEscaped};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::earth_db;
use crate::layout;
use crate::AppState;

const EARTH_BOOTSTRAP: &str = r#"
<script type="module">
  import { mountEarthGlobe } from '/static/js/earth-globe.js';
  const host = document.getElementById('earth-globe');
  if (host && host.dataset.mounted !== '1') {
    host.dataset.mounted = '1';
    mountEarthGlobe(host);
  }
</script>
"#;

/// `GET /earth`
pub async fn page() -> impl IntoResponse {
    layout::page("Earth", earth_markup())
}

pub fn earth_markup() -> Markup {
    html! {
        section class="earth" {
            header class="essay-header" {
                p class="eyebrow" { "Deep time · geology, fossils, evolution" }
                h1 { "Earth through time" }
                p class="lede" {
                    "A globe of reconstructed continents from 1.8 billion years ago to now, "
                    "with fossil occurrences from the Paleobiology Database. Slide the age "
                    "to watch plates assemble Rodinia, Gondwana, and Pangaea."
                }
            }
            div class="earth-globe" id="earth-globe" {
                canvas aria-label="Interactive globe of reconstructed continents through geologic time" {}
                div class="earth-hud" {
                    div class="earth-time" {
                        label for="earth-ma" { "Age" }
                        input id="earth-ma" type="range" min="0" max="1800" step="10" value="0"
                            aria-valuemin="0" aria-valuemax="1800" aria-valuenow="0";
                        p class="earth-time-readout" data-earth-readout { "0 Ma · present" }
                    }
                    div class="earth-search" {
                        label for="earth-taxon" { "Fossil taxon" }
                        input id="earth-taxon" type="search" autocomplete="off"
                            placeholder="Tyrannosaurus" maxlength="80";
                        ul class="earth-taxon-results" data-earth-results hidden {}
                    }
                }
                p class="earth-status" data-earth-status { "Loading globe…" }
                (PreEscaped(EARTH_BOOTSTRAP))
            }
            p class="earth-cite" {
                "Continents: Cao et al. 2024 via the GPlates Web Service (CC-BY). "
                "Time: ICS International Chronostratigraphic Chart (CC-BY 4.0). "
                "Fossils: Paleobiology Database (CC-BY 4.0). "
                "Rocks and environments: Macrostrat (CC-BY 4.0). "
                "This is not Google Earth imagery — deep-time maps are reconstructed plates, not satellite photos. "
                a href="https://github.com/julerex/fossall" { "Sources and schema" }
                " in the repo."
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MaQuery {
    pub ma: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TaxaQuery {
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OccQuery {
    pub ma: Option<f64>,
    pub taxon_id: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

fn json_err(status: StatusCode, msg: &str) -> Response {
    (
        status,
        Json(ApiError {
            error: msg.to_string(),
        }),
    )
        .into_response()
}

pub fn parse_ma(raw: Option<f64>) -> Result<f64, &'static str> {
    let ma = raw.ok_or("ma is required")?;
    if !ma.is_finite() || !(0.0..=4600.0).contains(&ma) {
        return Err("ma must be a number between 0 and 4600");
    }
    Ok(ma)
}

pub fn sanitize_taxon_query(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphabetic() || *c == ' ' || *c == '.' || *c == '-')
        .take(80)
        .collect::<String>()
        .trim()
        .to_string()
}

/// `GET /api/earth/timescale`
pub async fn timescale(State(state): State<AppState>) -> Response {
    let Some(pool) = &state.earth else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "earth database is not connected; set EARTH_DATABASE_URL (see docs/DATABASE.md)",
        );
    };
    match earth_db::timescale(pool).await {
        Ok(units) => Json(json!({ "units": units })).into_response(),
        Err(err) => {
            tracing::error!(?err, "timescale query failed");
            json_err(StatusCode::SERVICE_UNAVAILABLE, "could not read timescale")
        }
    }
}

/// `GET /api/earth/continents?ma=`
pub async fn continents(State(state): State<AppState>, Query(q): Query<MaQuery>) -> Response {
    let ma = match parse_ma(q.ma) {
        Ok(ma) => ma,
        Err(msg) => return json_err(StatusCode::BAD_REQUEST, msg),
    };
    let Some(pool) = &state.earth else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "earth database is not connected; set EARTH_DATABASE_URL (see docs/DATABASE.md)",
        );
    };
    match load_continents(pool, ma).await {
        Ok(body) => Json(body).into_response(),
        Err(err) => {
            tracing::error!(?err, "continents query failed");
            json_err(StatusCode::SERVICE_UNAVAILABLE, "could not read continents")
        }
    }
}

async fn load_continents(pool: &sqlx::PgPool, ma: f64) -> Result<Value, sqlx::Error> {
    let interval = earth_db::interval_at(pool, ma).await?;
    let (time_ma, rows) = earth_db::continents(pool, ma).await?;
    let features: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "type": "Feature",
                "properties": { "kind": row.feature_code },
                "geometry": row.geom
            })
        })
        .collect();
    Ok(json!({
        "requested_ma": ma,
        "time_ma": time_ma,
        "interval": interval,
        "type": "FeatureCollection",
        "features": features
    }))
}

/// `GET /api/earth/taxa?q=`
pub async fn taxa(State(state): State<AppState>, Query(q): Query<TaxaQuery>) -> Response {
    let Some(pool) = &state.earth else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "earth database is not connected; set EARTH_DATABASE_URL (see docs/DATABASE.md)",
        );
    };
    let q = sanitize_taxon_query(q.q.as_deref().unwrap_or(""));
    if q.len() < 2 {
        return Json(json!({ "taxa": [] })).into_response();
    }
    match earth_db::search_taxa(pool, &q, 20).await {
        Ok(taxa) => Json(json!({ "taxa": taxa })).into_response(),
        Err(err) => {
            tracing::error!(?err, "taxon search failed");
            json_err(StatusCode::SERVICE_UNAVAILABLE, "could not search taxa")
        }
    }
}

/// `GET /api/earth/occurrences?ma=&taxon_id=&limit=`
pub async fn occurrences(State(state): State<AppState>, Query(q): Query<OccQuery>) -> Response {
    let ma = match parse_ma(q.ma) {
        Ok(ma) => ma,
        Err(msg) => return json_err(StatusCode::BAD_REQUEST, msg),
    };
    let Some(pool) = &state.earth else {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "earth database is not connected; set EARTH_DATABASE_URL (see docs/DATABASE.md)",
        );
    };
    let limit = q.limit.unwrap_or(500);
    match earth_db::occurrences(pool, ma, q.taxon_id, limit).await {
        Ok(points) => Json(json!({
            "ma": ma,
            "taxon_id": q.taxon_id,
            "count": points.len(),
            "occurrences": points
        }))
        .into_response(),
        Err(err) => {
            tracing::error!(?err, "occurrences query failed");
            json_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "could not read occurrences",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ma_rejects_missing_and_out_of_range() {
        assert!(parse_ma(None).is_err());
        assert!(parse_ma(Some(f64::NAN)).is_err());
        assert!(parse_ma(Some(-1.0)).is_err());
        assert!(parse_ma(Some(9000.0)).is_err());
        assert_eq!(parse_ma(Some(0.0)).unwrap(), 0.0);
        assert_eq!(parse_ma(Some(250.0)).unwrap(), 250.0);
    }

    #[test]
    fn sanitize_taxon_query_strips_noise() {
        assert_eq!(
            sanitize_taxon_query(" Tyrannosaurus rex! "),
            "Tyrannosaurus rex"
        );
        assert_eq!(sanitize_taxon_query("H. sapiens"), "H. sapiens");
        assert_eq!(sanitize_taxon_query(""), "");
        assert!(sanitize_taxon_query(&"a".repeat(200)).len() <= 80);
    }

    #[test]
    fn earth_markup_has_globe_and_slider() {
        let html = earth_markup().into_string();
        assert!(html.contains("id=\"earth-globe\""));
        assert!(html.contains("id=\"earth-ma\""));
        assert!(html.contains("id=\"earth-taxon\""));
        assert!(html.contains("Cao et al. 2024"));
        assert!(html.contains("Paleobiology Database"));
        assert!(html.contains("mountEarthGlobe"));
        assert!(!html.contains("DATABASE_URL is unset"));
    }
}
