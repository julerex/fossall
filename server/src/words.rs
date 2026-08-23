//! `GET /words` — every five-letter string for a two-letter start.

use std::collections::HashSet;
use std::str;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use maud::{html, Markup};
use serde::Deserialize;

use crate::db;
use crate::layout;
use crate::AppState;

const ALPHA: usize = 26;

/// Every `[a-z]` triple that can follow a two-letter prefix.
pub const COMBINATIONS: usize = ALPHA * ALPHA * ALPHA;

/// Cells per table row.
pub const TABLE_COLS: usize = 5;

#[derive(Debug, Deserialize, Default)]
pub struct WordsQuery {
    pub q: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub letters: [u8; 5],
    pub real: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Results {
    Unavailable,
    Error,
    Prompt {
        total: i64,
        prefix: String,
    },
    Grid {
        prefix: String,
        real_count: usize,
        cells: Vec<Cell>,
    },
}

/// Keep only ASCII letters, lowercase, at most two characters.
pub fn sanitize_prefix(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .take(2)
        .collect()
}

/// Map `index` in `0..COMBINATIONS` to `prefix` plus a lexicographic triple.
pub fn combo_at(prefix: &str, index: usize) -> [u8; 5] {
    let bytes = prefix.as_bytes();
    debug_assert_eq!(bytes.len(), 2);
    debug_assert!(index < COMBINATIONS);
    let a = (index / (ALPHA * ALPHA)) as u8;
    let b = ((index / ALPHA) % ALPHA) as u8;
    let c = (index % ALPHA) as u8;
    [bytes[0], bytes[1], b'a' + a, b'a' + b, b'a' + c]
}

pub fn cells(prefix: &str, real: &HashSet<[u8; 5]>) -> Vec<Cell> {
    (0..COMBINATIONS)
        .map(|index| {
            let letters = combo_at(prefix, index);
            Cell {
                letters,
                real: real.contains(&letters),
            }
        })
        .collect()
}

fn cell_text(letters: &[u8; 5]) -> &str {
    str::from_utf8(letters).unwrap_or("")
}

fn word_key(word: &str) -> Option<[u8; 5]> {
    word.as_bytes().try_into().ok()
}

pub async fn words(
    State(state): State<AppState>,
    Query(query): Query<WordsQuery>,
) -> impl IntoResponse {
    let prefix = sanitize_prefix(query.q.as_deref().unwrap_or(""));
    let Some(pool) = &state.db else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            layout::page("Words", words_page(&prefix, &Results::Unavailable)),
        );
    };

    match load_results(pool, &prefix).await {
        Ok(results) => (
            StatusCode::OK,
            layout::page("Words", words_page(&prefix, &results)),
        ),
        Err(err) => {
            tracing::error!(?err, "words query failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                layout::page("Words", words_page(&prefix, &Results::Error)),
            )
        }
    }
}

async fn load_results(pool: &sqlx::PgPool, prefix: &str) -> Result<Results, sqlx::Error> {
    if prefix.len() < 2 {
        let total = db::count_words(pool).await?;
        return Ok(Results::Prompt {
            total,
            prefix: prefix.to_string(),
        });
    }
    let words = db::words_with_prefix(pool, prefix).await?;
    let real: HashSet<[u8; 5]> = words.iter().filter_map(|word| word_key(word)).collect();
    let real_count = real.len();
    Ok(Results::Grid {
        prefix: prefix.to_string(),
        real_count,
        cells: cells(prefix, &real),
    })
}

fn words_page(prefix: &str, results: &Results) -> Markup {
    html! {
        section class="words" {
            header class="essay-header" {
                p class="eyebrow" { "ENABLE word list · Postgres" }
                h1 { "Five-letter words" }
                p class="lede" {
                    "Type two letters. Every five-letter string that starts with them is listed "
                    "below — 17,576 of them — and the ones in the public-domain ENABLE list "
                    "are highlighted in green."
                }
            }
            form class="words-form"
                action="/words"
                method="get"
                hx-get="/words"
                hx-trigger="input delay:200ms from:input, submit"
                hx-target="#word-results"
                hx-select="#word-results"
                hx-push-url="true"
            {
                label for="word-q" { "First two letters" }
                input id="word-q"
                    class="words-search"
                    type="search"
                    name="q"
                    value=(prefix)
                    maxlength="2"
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="st"
                    aria-describedby="word-results";
            }
            (results_markup(results))
        }
    }
}

pub fn results_markup(results: &Results) -> Markup {
    html! {
        div id="word-results" {
            @match results {
                Results::Unavailable => {
                    p class="words-error" {
                        "The word list is not connected in this process. "
                        "Set DATABASE_URL (see docs/DATABASE.md) and restart."
                    }
                }
                Results::Error => {
                    p class="words-error" {
                        "Could not read the word list from Postgres. Try again shortly."
                    }
                }
                Results::Prompt { total, prefix } => {
                    p class="words-meta" {
                        @if prefix.is_empty() {
                            "Type two letters to see every five-letter string that starts with them. "
                            span class="tabular" { (total) }
                            " words are in ENABLE."
                        } @else {
                            "Type a second letter after “" (prefix) "”."
                        }
                    }
                }
                Results::Grid { prefix, real_count, cells } => {
                    p class="words-meta" {
                        span class="tabular" { (real_count) }
                        " of "
                        span class="tabular" { (COMBINATIONS) }
                        " strings starting with “"
                        (prefix)
                        "” are in ENABLE."
                    }
                    div class="table-wrap" {
                        table class="word-table" {
                            tbody {
                                @for row in cells.chunks(TABLE_COLS) {
                                    tr {
                                        @for cell in row {
                                            @if cell.real {
                                                td class="word-real" { (cell_text(&cell.letters)) }
                                            } @else {
                                                td { (cell_text(&cell.letters)) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_prefix_lowercases_and_strips_non_alpha() {
        assert_eq!(sanitize_prefix("St-2are!"), "st");
    }

    #[test]
    fn sanitize_prefix_truncates_to_two() {
        assert_eq!(sanitize_prefix("STACKS"), "st");
    }

    #[test]
    fn sanitize_prefix_empty_and_symbols() {
        assert_eq!(sanitize_prefix(""), "");
        assert_eq!(sanitize_prefix("123 %_"), "");
        assert_eq!(sanitize_prefix("A"), "a");
    }

    #[test]
    fn combo_at_starts_at_aaa() {
        assert_eq!(combo_at("st", 0), *b"staaa");
    }

    #[test]
    fn combo_at_ends_at_zzz() {
        assert_eq!(combo_at("st", COMBINATIONS - 1), *b"stzzz");
    }

    #[test]
    fn combo_at_is_lexicographic() {
        assert_eq!(combo_at("st", 1), *b"staab");
        assert_eq!(combo_at("st", ALPHA - 1), *b"staaz");
        assert_eq!(combo_at("st", ALPHA), *b"staba");
        assert_eq!(combo_at("st", ALPHA * ALPHA), *b"stbaa");
        assert!(combo_at("st", 0) < combo_at("st", 1));
    }

    #[test]
    fn cells_covers_every_triple() {
        let grid = cells("ab", &HashSet::new());
        assert_eq!(grid.len(), COMBINATIONS);
        assert_eq!(grid[0].letters, *b"abaaa");
        assert_eq!(grid[COMBINATIONS - 1].letters, *b"abzzz");
        assert!(grid.iter().all(|cell| !cell.real));
    }

    #[test]
    fn cells_marks_real_words_only() {
        let real = HashSet::from([*b"aback", *b"stare"]);
        let grid = cells("ab", &real);
        let marked: Vec<_> = grid.iter().filter(|cell| cell.real).collect();
        assert_eq!(marked.len(), 1);
        assert_eq!(marked[0].letters, *b"aback");
    }

    #[test]
    fn results_markup_prompt_asks_for_two_letters() {
        let html = results_markup(&Results::Prompt {
            total: 8636,
            prefix: String::new(),
        })
        .into_string();
        assert!(html.contains("id=\"word-results\""));
        assert!(html.contains("Type two letters"));
        assert!(html.contains("8636"));
        assert!(!html.contains("word-table"));
    }

    #[test]
    fn results_markup_prompt_asks_for_second_letter() {
        let html = results_markup(&Results::Prompt {
            total: 8636,
            prefix: "s".into(),
        })
        .into_string();
        assert!(html.contains("Type a second letter after “s”"));
        assert!(!html.contains("word-table"));
    }

    #[test]
    fn results_markup_grid_highlights_real_words() {
        let real = HashSet::from([*b"stare", *b"steal"]);
        let html = results_markup(&Results::Grid {
            prefix: "st".into(),
            real_count: 2,
            cells: cells("st", &real),
        })
        .into_string();
        assert!(html.contains("word-table"));
        assert!(html.contains("staaa"));
        assert!(html.contains("stzzz"));
        assert!(html.contains(">2</span>"));
        assert!(html.contains("starting with “st”"));
        assert!(html.contains(&COMBINATIONS.to_string()));
        assert_eq!(html.matches("word-real").count(), 2);
        assert!(html.contains("stare"));
        assert!(html.contains("steal"));
        let rows = (COMBINATIONS + TABLE_COLS - 1) / TABLE_COLS;
        assert_eq!(html.matches("<tr>").count(), rows);
    }

    #[test]
    fn results_markup_grid_without_real_words() {
        let html = results_markup(&Results::Grid {
            prefix: "qz".into(),
            real_count: 0,
            cells: cells("qz", &HashSet::new()),
        })
        .into_string();
        assert!(html.contains(">0</span>"));
        assert!(html.contains("starting with “qz”"));
        assert!(html.contains("qzaaa"));
        assert!(!html.contains("word-real"));
    }

    #[test]
    fn results_markup_unavailable_and_errors() {
        let missing = results_markup(&Results::Unavailable).into_string();
        assert!(missing.contains("DATABASE_URL"));

        let err = results_markup(&Results::Error).into_string();
        assert!(err.contains("Could not read the word list"));
    }
}
