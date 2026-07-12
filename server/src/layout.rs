//! Shared HTML document shell (nav, scripts, footer).

use maud::{html, Markup, DOCTYPE};

/// Full HTML page with site chrome.
pub fn page(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · Fossall" }
                meta name="description" content="FOSS all the things — experiments in open, low-cost mobility and software.";
                link rel="icon" href="/static/favicon.svg" type="image/svg+xml";
                link rel="stylesheet" href="/static/css/style.css";
                script src="/static/htmx.min.js" defer {}
                // Import map for optional Three.js modules (3D sketch on /rv).
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
                        a href="/homeprices" { "Home prices" }
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
