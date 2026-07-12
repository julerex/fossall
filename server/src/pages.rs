//! Page handlers and essay content.

use axum::response::IntoResponse;
use maud::{html, Markup, PreEscaped};

use crate::layout;

/// `GET /` — short pitch and links.
pub async fn home() -> impl IntoResponse {
    layout::page(
        "Home",
        html! {
            section class="hero" {
                h1 { "FOSS all the things" }
                p class="lede" {
                    "Fossall is a place for open, low-cost experiments — software first, "
                    "and eventually the hardware and systems that should not require a "
                    "closed stack to understand or improve."
                }
                p {
                    a class="btn" href="/rv" hx-boost="true" {
                        "Read: low-cost electric self-driving container RV →"
                    }
                }
            }
            section class="cards" {
                article {
                    h2 { "Why here" }
                    p {
                        "Start small: a public site that is almost entirely Rust — "
                        "server-rendered HTML, HTMX for navigation, and WASM only where "
                        "client compute actually helps."
                    }
                }
                article {
                    h2 { "First thread" }
                    p {
                        "Is a recreational vehicle about the size of a shipping container, "
                        "fully electric and fully self-driving, possible at a low cost? "
                        "The essay sketches constraints, not a product."
                    }
                }
            }
        },
    )
}

/// `GET /rv` — feasibility essay + WASM pack estimator.
pub async fn rv_essay() -> impl IntoResponse {
    layout::page(
        "Container EV-RV",
        html! {
            article class="essay" {
                header class="essay-header" {
                    p class="eyebrow" { "Feasibility sketch · not a product pitch" }
                    h1 { "A low-cost, fully electric, fully self-driving RV the size of a shipping container" }
                    p class="lede" {
                        "Could you build a recreational vehicle that fits roughly in a "
                        "shipping-container envelope, runs only on batteries, drives itself, "
                        "and still lands at a price ordinary people might actually pay? "
                        "This page is an open look at the hard parts — form factor, energy, "
                        "autonomy, and cost — not a claim that the answer is yes."
                    }
                }

                section {
                    h2 { "1. The premise" }
                    p {
                        "A standard ISO container is a brutal design brief: about "
                        strong { "2.4 m wide" } ", roughly " strong { "2.6 m high" }
                        " externally for a high-cube, and either " strong { "6 m (20′)" }
                        " or " strong { "12 m (40′)" } " long. As a "
                        em { "living" } " volume that is cramped; as a "
                        em { "road vehicle" } " envelope it is already wider and taller "
                        "than many passenger cars, and a full 40′ box is truck territory."
                    }
                    p {
                        "“Fully electric” means no diesel generator as a crutch for range. "
                        "“Fully self-driving” means the hard version of autonomy — not "
                        "adaptive cruise on a good day, but the expectation that the vehicle "
                        "can move people and their stuff without a licensed pilot in the loop "
                        "for ordinary trips. “Low cost” is the third constraint that makes "
                        "the first two interesting: if the only way to close the triangle is "
                        "a six-figure battery and a robotaxi-grade sensor suite, you have a "
                        "lab demo, not a recreational product."
                    }
                }

                section {
                    h2 { "2. Form factor: container as constraint" }
                    p {
                        "Road rules, not container standards, decide what you can actually drive. "
                        "In much of North America, legal width without special permits is about "
                        "2.6 m (8′6″); a container’s ~2.44 m exterior width fits, but once you "
                        "add mirrors, fenders, insulation, and structure, the “shipping box on "
                        "wheels” fantasy meets chassis and body engineering immediately."
                    }
                    p {
                        "Height is similar: high-cube exterior height is already near the "
                        "comfort zone for bridges and parking structures. Length is a "
                        "spectrum: a 20′ class living module is more “tiny home on a "
                        "purpose-built EV skateboard”; a 40′ class vehicle is closer to a "
                        "Class A coach or box truck in mass, tire count, and licensing."
                    }
                    p {
                        "Mass is the quiet killer. Steel containers are heavy "
                        em { "before" } " you add battery, motors, suspension, furniture, "
                        "water, and people. A low-cost design almost certainly does "
                        strong { "not" } " reuse a steel intermodal box as the primary "
                        "structure; it uses the " em { "dimensions" } " as a size target "
                        "while building a lighter monocoque or spaceframe body meant for "
                        "the road."
                    }
                    ul {
                        li { "Envelope: think high-cube 20′ first; 40′ only if you accept truck-like complexity." }
                        li { "Body: composite or aluminum skin over a purpose-built EV chassis, not a raw container." }
                        li { "Interior: fixed wet cell + transformable living space; volume is the scarce resource." }
                    }
                }

                section {
                    h2 { "3. Energy: batteries vs a rolling house" }
                    p {
                        "A boxy RV is aerodynamically rude. At highway speed, drag dominates. "
                        "Ballpark consumption for a large, tall electric vehicle is often "
                        "quoted in the " strong { "300–600 Wh/km" } " range depending on "
                        "speed, mass, and weather — several times a compact car. That means "
                        "range is expensive in both dollars and kilograms."
                    }
                    p {
                        "Order-of-magnitude pack math (pack-level, not cell fantasy): "
                        "roughly " strong { "150–180 Wh/kg" } " usable pack energy density "
                        "and on the order of " strong { "$100–150 / kWh" } " pack cost in "
                        "the mid-2020s, trending down but not free. A 100 kWh pack is then "
                        "hundreds of kilograms and five figures of cost " em { "before" }
                        " integration, thermal management, and structure."
                    }
                    p {
                        "Use the estimator below (implemented in Rust WASM) to feel the "
                        "trade: larger packs buy range and cost mass and money; they do not "
                        "fix the aero tax of a house-shaped vehicle."
                    }

                    (pack_estimator_widget())

                    p {
                        "Charging and duty cycle matter as much as pack size. A recreational "
                        "vehicle that mostly sits at a campground on shore power has a "
                        "different battery story than one expected to hop 400 km between "
                        "national parks every day. “Low cost” might mean "
                        strong { "modest range + ubiquitous destination charging" }
                        " rather than Tesla-like road-trip buffer for a brick."
                    }
                }

                section {
                    h2 { "4. Autonomy: the expensive last 1%" }
                    p {
                        "Highway lane-keeping and adaptive cruise are commodity-ish. "
                        "Full self-driving in the sense of unsupervised operation on public "
                        "roads — weather, construction, pedestrians, parking lots, dirt "
                        "access roads to campsites — is still the frontier where companies "
                        "burn billions."
                    }
                    p {
                        "Sensor suites (cameras, radar, optional lidar), compute, mapping, "
                        "validation, and liability dominate cost long after the motors are "
                        "chosen. For an RV, low-speed yard maneuvering and remote "
                        "tele-assist might be more valuable than unsupervised city driving, "
                        "and far cheaper to make honest. Regulatory regimes also differ by "
                        "jurisdiction; a product that is legal to sell as “fully "
                        "self-driving” in one market may be “driver assist only” in another."
                    }
                    ul {
                        li { "Cheap-ish: ADAS for highway fatigue reduction and parking assist." }
                        li { "Hard: unsupervised door-to-door trips with sleeping occupants." }
                        li { "Maybe FOSS-adjacent: open tooling for mapping, simulation, and driver-out-of-loop monitoring — not the full stack on day one." }
                    }
                }

                section {
                    h2 { "5. Cost stack: where “low cost” breaks" }
                    p {
                        "A crude stack for a container-scale EV RV might look like:"
                    }
                    ol {
                        li { strong { "Rolling chassis" } " — motors, inverters, suspension, brakes, thermal." }
                        li { strong { "Battery pack" } " — cells, modules, BMS, enclosure, crash structure." }
                        li { strong { "Body & interior" } " — insulated shell, glazing, wet cell, furniture." }
                        li { strong { "Autonomy & electronics" } " — compute, sensors, wiring, HMI." }
                        li { strong { "Certification & support" } " — crash, emissions-adjacent rules, service network." }
                    }
                    p {
                        "Battery and autonomy are the two cliffs. Body and chassis can be "
                        "value-engineered; liability and validation cannot be wishcast away. "
                        "“Low cost” almost certainly means "
                        strong { "narrowing the mission" } ": shorter range, human still "
                        "responsible for driving for years, shared platforms with commercial "
                        "EV vans or skateboards, and interior kit that is modular rather "
                        "than yacht-grade."
                    }
                }

                section {
                    h2 { "6. Open questions and a FOSS angle" }
                    p {
                        "What could actually be open?"
                    }
                    ul {
                        li {
                            strong { "Software" } " — energy management UIs, trip planners "
                            "tuned for RV duty cycles, open diagnostics, non-cloud lock-in "
                            "for vehicle telemetry the owner actually owns."
                        }
                        li {
                            strong { "Design" } " — published dimensions, mass budgets, and "
                            "interior modules others can fork (the way open hardware frames "
                            "spread in other domains)."
                        }
                        li {
                            strong { "Not magically open" } " — cell factories, full "
                            "autonomy stacks trained on proprietary fleets, and type "
                            "approval. Fossall’s bet is not that the whole vehicle is a git "
                            "repo; it is that the closed middle of mobility should shrink."
                        }
                    }
                    p {
                        "A shipping-container-sized electric, self-driving RV that is "
                        em { "cheap" } " is probably impossible if you demand all three "
                        "adjectives at once in their strongest form. A "
                        strong { "container-scale electric camper with strong driver "
                        "assistance, honest range, and open software around it" }
                        " is a much more interesting near-term target — and a better place "
                        "to start building in public."
                    }
                }

                section class="closing" {
                    h2 { "Next" }
                    p {
                        "This site is the first brick. If the idea is worth pursuing, the "
                        "follow-ons are concrete: mass budgets, skateboard platform options, "
                        "and a public cost model others can argue with. Until then: FOSS "
                        "all the things you can, and be honest about the rest."
                    }
                    p {
                        a class="btn secondary" href="/" hx-boost="true" { "← Back home" }
                    }
                }
            }
        },
    )
}

/// Interactive pack estimator shell; logic runs in `/wasm/fossall_wasm.js`.
fn pack_estimator_widget() -> Markup {
    html! {
        aside class="widget" id="pack-estimator" {
            h3 { "Rough pack estimator" }
            p class="widget-note" {
                "Client-side Rust (WASM). Order-of-magnitude only — ~160 Wh/kg pack, "
                "~$120/kWh, ~450 Wh/km for a boxy highway RV."
            }
            label for="pack-kwh" {
                "Pack size: "
                strong id="pack-kwh-label" { "100" }
                " kWh"
            }
            input type="range" id="pack-kwh" min="20" max="400" step="10" value="100"
                aria-valuemin="20" aria-valuemax="400" aria-valuenow="100";
            p class="widget-out" id="pack-out" { "Loading estimator…" }
            (PreEscaped(PACK_ESTIMATOR_BOOTSTRAP))
        }
    }
}

/// Minimal module script: load WASM and wire the range input. No app logic in JS.
const PACK_ESTIMATOR_BOOTSTRAP: &str = r#"
<script type="module">
  import init, { estimate_pack, default_kwh } from '/wasm/fossall_wasm.js';
  const out = document.getElementById('pack-out');
  const slider = document.getElementById('pack-kwh');
  const label = document.getElementById('pack-kwh-label');
  try {
    await init();
    const apply = () => {
      const kwh = Number(slider.value);
      label.textContent = String(kwh);
      slider.setAttribute('aria-valuenow', String(kwh));
      out.textContent = estimate_pack(kwh);
    };
    slider.value = String(default_kwh());
    slider.addEventListener('input', apply);
    apply();
  } catch (e) {
    out.textContent = 'WASM estimator unavailable (build with `make build-wasm`).';
    console.error(e);
  }
</script>
"#;

/// `GET /health` — liveness for Fly.
pub async fn health() -> &'static str {
    "ok"
}
