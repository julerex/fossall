//! Shared HTML document shell (nav, scripts, footer).

use maud::{html, Markup, DOCTYPE};

/// Allow 422 responses from `/demo` compose to swap error markup.
const HTMX_DEMO_CONFIG: &str = r#"{"responseHandling":[{"code":"204","swap":false},{"code":"422","swap":true},{"code":"[23]..","swap":true},{"code":"[45]..","swap":false,"error":true}]}"#;

/// Full HTML page with site chrome.
pub fn page(title: &str, body: Markup) -> Markup {
    shell(title, body, false)
}

/// `/demo` shell — vendors `_hyperscript` and enables 422 swaps.
pub fn demo_page(title: &str, body: Markup) -> Markup {
    shell(title, body, true)
}

fn shell(title: &str, body: Markup, hyperscript: bool) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · Fossall" }
                meta name="description" content="FOSS all the things — experiments in open, low-cost mobility and software.";
                link rel="icon" href="/static/favicon.svg" type="image/svg+xml";
                link rel="stylesheet" href="/static/css/style.css?v=6";
                @if hyperscript {
                    meta name="htmx-config" content=(HTMX_DEMO_CONFIG);
                }
                script src="/static/htmx.min.js" defer {}
                @if hyperscript {
                    script src="/static/_hyperscript.min.js" defer {}
                }
                // Import map for optional Three.js modules (3D sketches on /rv and /earth).
                script type="importmap" {
                    (maud::PreEscaped(r#"{
  "imports": {
    "three": "https://cdn.jsdelivr.net/npm/three@0.170.0/build/three.module.js",
    "three/addons/": "https://cdn.jsdelivr.net/npm/three@0.170.0/examples/jsm/"
  }
}"#))
                }
            }
            body {
                header class="site-header" {
                    a class="logo" href="/" hx-boost="true" { "Fossall" }
                    nav hx-boost="true" {
                        a href="/" { "Home" }
                        a href="/rv" { "Container EV-RV" }
                        a href="/earth" hx-boost="false" { "Earth" }
                        a href="/homeprices" { "Home prices" }
                        a href="/words" { "Words" }
                        a href="/demo" hx-boost="false" { "Demo" }
                    }
                }
                main { (body) }
                footer class="site-footer" {
                    p {
                        "FOSS ALL THE THINGS. Built with Rust, HTMX, and WASM. "
                        a href="https://github.com/julerex/fossall" { "Source" }
                    }
                }
            }
        }
    }
}
