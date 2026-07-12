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
                        "The essay sketches constraints, solar yield, and a Tesla-scale "
                        "cost model — not a product brochure."
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
                        "solar, autonomy, and cost — including a Tesla-scale cost estimate "
                        "for a mostly parked, short-range mission."
                    }
                }

                section {
                    h2 { "1. The premise" }
                    p {
                        "A standard ISO container is a brutal design brief: about "
                        strong { "2.4 m wide" } ", roughly " strong { "2.9 m high" }
                        " externally for a high-cube, and either " strong { "6 m (20′)" }
                        " or " strong { "12 m (40′)" } " long. As a "
                        em { "living" } " volume that is still modest; as a "
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
                    p {
                        "The rest of this page narrows the mission so the triangle can close. "
                        "Assume the vehicle " strong { "drives only short distances" }
                        ", " strong { "spends most of its time parked" } ", and is "
                        strong { "software-limited to about 80 km/h" } ". Range is not a "
                        "hero feature. Neither is cargo capacity. The product is closer to a "
                        em { "mobile tiny home on a multi-axle skateboard" } " than to a "
                        "Class A coach that pretends it is a highway car."
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
                        "Class A coach or box truck in mass, tire count, and licensing — "
                        "but under a short-range, mostly parked duty cycle, that length "
                        "buys living volume without forcing a Semi-sized battery."
                    }
                    p {
                        "Mass is the quiet killer. Steel containers are heavy "
                        em { "before" } " you add battery, motors, suspension, furniture, "
                        "water, and people. A low-cost design almost certainly does "
                        strong { "not" } " reuse a steel intermodal box as the primary "
                        "structure; it uses the " em { "dimensions" } " as a size target "
                        "while building a lighter monocoque or spaceframe body meant for "
                        "the road. ISO corner-casting strength is for stacking freight on ships — "
                        "not a requirement for a road camper that never sees a crane."
                    }
                    ul {
                        li { "Envelope: 40′ high-cube exterior dimensions as the living target; structure is purpose-built, not a scrap container." }
                        li { "Body: composite or aluminum skin over a multi-axle EV skateboard — no steel ISO box mass tax." }
                        li { "Interior: fixed wet cell + transformable living space; volume is the scarce resource." }
                        li { "Curb mass ballpark: roughly 4.5–7.5 t empty of fluids; loaded maybe 6–9 t — far below a freight Semi GCW." }
                    }
                    p {
                        "One credible road form factor is the "
                        em { "cabless hauler" } " pattern: a low electric skateboard with multi-axle "
                        "e-drive, corner sensor pods, and a lock interface for a 40′ box — the same "
                        "family of idea as commercial autonomous container haulers, but with the "
                        "box fitted as a living module (windows, door, solar, wet cell) rather than "
                        "sealed freight. Explore a rough interactive sketch below."
                    }

                    (rv_model_widget())
                }

                section {
                    h2 { "3. Energy: batteries vs a rolling house" }
                    p {
                        "A boxy RV is aerodynamically rude. At true highway speed, drag dominates. "
                        "Ballpark consumption for a large, tall electric vehicle is often "
                        "quoted in the " strong { "300–600 Wh/km" } " range depending on "
                        "speed, mass, and weather — several times a compact car. Cap the "
                        "vehicle at " strong { "80 km/h" } " and the worst of the aero tax "
                        "eases; " strong { "350–550 Wh/km" } " is a fair planning band for a "
                        "tall ~7 t brick on short hops. That is still expensive energy if you "
                        "insist on 500 km of range — but we do not."
                    }
                    p {
                        "Order-of-magnitude pack math (pack-level, not cell fantasy): "
                        "roughly " strong { "150–180 Wh/kg" } " usable pack energy density "
                        "and on the order of " strong { "$80–120 / kWh" } " pack cost at "
                        "volume in the mid-2020s, trending down but not free. A 100 kWh pack is then "
                        "hundreds of kilograms and low-to-mid five figures of cost " em { "before" }
                        " integration, thermal management, and structure."
                    }
                    p {
                        "Under a " strong { "mostly parked" } " duty cycle the traction pack "
                        "is also the house battery. You do not need a Semi pack. You need "
                        "enough energy for a few days of living plus one reposition:"
                    }
                    ul {
                        li { "60 kWh → ~130 km at 450 Wh/km — local only, tight." }
                        li { strong { "80–100 kWh → ~180–220 km" } " — the sweet spot for short moves." }
                        li { "120 kWh → ~270 km — optional “weekend hop” buffer." }
                    }
                    p {
                        "A 30 km site-to-site hop is only ~10–15 kWh. The hard problem is "
                        "not driving; it is " em { "parked loads" } " — fridge, lights, pumps, "
                        "and especially HVAC. That is where the roof solar (next section) "
                        "earns its keep."
                    }
                    p {
                        "Use the estimator below (implemented in Rust WASM) to feel the "
                        "trade: larger packs buy range and cost mass and money; they do not "
                        "fix the aero tax of a house-shaped vehicle. The default ~90 kWh "
                        "is the short-range product case, not a coast-to-coast fantasy."
                    }

                    (pack_estimator_widget())
                }

                section {
                    h2 { "4. Solar: the roof is the point" }
                    p {
                        "A 40′ high-cube roof is one of the few places where the container "
                        "envelope is an " em { "advantage" } " over a passenger EV. Exterior "
                        "plan area is about " strong { "12.2 × 2.44 m ≈ 30 m²" } ". After "
                        "rails, vents, edge setback, and imperfect flat-roof tilt, usable PV "
                        "area is roughly " strong { "20–24 m²" } "."
                    }
                    p {
                        "Modern modules around " strong { "22% efficiency" } " deliver on the "
                        "order of " strong { "220 W/m²" } " under standard test conditions. "
                        "That is a " strong { "~4.5–5.3 kW peak" } " array on this roof — "
                        "not a decorative strip."
                    }
                    p {
                        "Real daily yield needs a system derate (heat, dirt, wiring, non-ideal "
                        "tilt). Using ~0.75 and a ~4.8 kW nameplate:"
                    }
                    div class="table-wrap" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "Climate" }
                                    th { "Peak sun hours" }
                                    th { "Daily harvest" }
                                }
                            }
                            tbody {
                                tr {
                                    td { "SW US / Mediterranean summer" }
                                    td { "5.5–6.5" }
                                    td { strong { "~20–23 kWh/day" } }
                                }
                                tr {
                                    td { "US average / southern Europe" }
                                    td { "4–5" }
                                    td { strong { "~14–18 kWh/day" } }
                                }
                                tr {
                                    td { "Northern Europe / cloudy winter" }
                                    td { "1.5–3" }
                                    td { strong { "~5–11 kWh/day" } }
                                }
                            }
                        }
                    }
                    p {
                        "What does a parked house actually burn?"
                    }
                    ul {
                        li { "Fridge + electronics + lights + pumps: roughly 2–5 kWh/day." }
                        li { "Heat-pump HVAC in mild weather: add ~5–12 kWh/day." }
                        li { "Hard AC or hard heating: add ~15–35 kWh/day — solar alone will not always cover it." }
                        li { "Sparse induction cooking and water heat: a few kWh more on active days." }
                    }
                    p {
                        "In mild climates with an efficient shell, roof solar can cover "
                        strong { "most or all" } " parked energy for much of the year. In "
                        "hot desert summers or northern winters it is a " strong { "buffer" }
                        ", not full independence — shore power, destination charging, or an "
                        "occasional drive-to-charge still matter. Either way, for a vehicle "
                        "that mostly sits, solar changes the pack story: you size the battery "
                        "for " em { "a few days of house + one move" } ", not for weeks of "
                        "generator-free highway touring."
                    }
                    p class="callout" {
                        "Energy story in one line: drive 30 km (~15 kWh), stay a week, take "
                        "~100 kWh from the sun in a decent climate, leave without ever needing "
                        "a 500 kWh Semi pack."
                    }
                }

                section {
                    h2 { "5. Autonomy: the expensive last 1%" }
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
                    p {
                        "If the builder is Tesla-scale, camera + onboard computer hardware is "
                        "already amortized across Model Y volume — the bill of materials is "
                        "cheap (order of " strong { "$1–2k" } " for the hardware stack). "
                        "The remaining cliffs are " strong { "validation, liability, and "
                        "selling unsupervised operation with sleeping occupants" } " — none "
                        "of which disappear because the body is a container envelope."
                    }
                    ul {
                        li { "Cheap-ish: ADAS for fatigue reduction, parking assist, and low-speed repositioning." }
                        li { "Hard: unsupervised door-to-door trips with sleeping occupants." }
                        li { "Hardware is not the bottleneck; honesty about the product label is." }
                    }
                }

                section {
                    h2 { "6. Cost estimate: Tesla-scale, short-range mission" }
                    p {
                        "Ignore the open-source framing for a moment and ask a different "
                        "question: " em { "what would this cost if a company that already "
                        "builds high-volume EVs and multi-axle electric trucks built it?" }
                        " Use the best-selling Tesla car as the cost DNA, and the Tesla Semi "
                        "as the truck architecture benchmark — then " strong { "delete most "
                        "of the Semi" } " because range, cargo, and speed are not the mission."
                    }

                    h3 { "Benchmarks" }
                    div class="table-wrap" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "Reference" }
                                    th { "Role here" }
                                    th { "Ballpark" }
                                }
                            }
                            tbody {
                                tr {
                                    td { strong { "Model Y" } }
                                    td { "Volume cost, pack, motors, FSD hardware" }
                                    td { "~$40–50k MSRP; ~60–81 kWh pack; ~2 t curb" }
                                }
                                tr {
                                    td { strong { "Tesla Semi" } }
                                    td { "Multi-axle e-drive architecture" }
                                    td { "~$260–300k; 548–822 kWh; ~800 kW peak; ~1.7 kWh/mi" }
                                }
                                tr {
                                    td { strong { "This vehicle" } }
                                    td { "Derated skateboard + living module" }
                                    td { "~1.5× Model Y energy; ~¼–⅓ Semi power; no cargo GCW" }
                                }
                            }
                        }
                    }
                    p {
                        "The Semi exists to pull 82,000 lb combination weight for hundreds of "
                        "miles. This RV does not. Continuous road load at 80 km/h for a tall "
                        "~7–8 t vehicle is on the order of " strong { "40–80 kW" } " on the flat; "
                        strong { "150–250 kW peak" } " covers hills and launch. That is one "
                        "strong passenger-EV motor class — not three heavy-truck motors at "
                        "800 kW. Pack target: " strong { "~90 kWh" } " (about one Model Y Long "
                        "Range class pack), not 500–800 kWh."
                    }

                    h3 { "Unit cost stack (volume production)" }
                    p {
                        "Assumptions: Tesla-like vertical integration and cell pricing "
                        "(~" strong { "$90/kWh" } " internal pack planning band), interior that "
                        "is minimal rather than yacht-grade, and volume on the order of "
                        strong { "10k+ units/year" } " for the base case. Pilot volumes are "
                        "materially more expensive per unit."
                    }
                    div class="table-wrap" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "Subsystem" }
                                    th { "Lean" }
                                    th { "Base" }
                                    th { "Fat" }
                                }
                            }
                            tbody {
                                tr {
                                    td { "Traction pack ~90 kWh" }
                                    td { "$7k" }
                                    td { "$9k" }
                                    td { "$12k" }
                                }
                                tr {
                                    td { "Motors + inverters (150–250 kW class)" }
                                    td { "$3k" }
                                    td { "$4.5k" }
                                    td { "$7k" }
                                }
                                tr {
                                    td { "Skateboard: structure, axles, suspension, brakes" }
                                    td { "$10k" }
                                    td { "$15k" }
                                    td { "$22k" }
                                }
                                tr {
                                    td { "Body shell (Al/composite, insulated, glazed)" }
                                    td { "$12k" }
                                    td { "$18k" }
                                    td { "$28k" }
                                }
                                tr {
                                    td { "Interior: wet cell, galley, furniture, tanks" }
                                    td { "$12k" }
                                    td { "$20k" }
                                    td { "$35k" }
                                }
                                tr {
                                    td { "HVAC, water heat, low-voltage house" }
                                    td { "$3k" }
                                    td { "$5k" }
                                    td { "$8k" }
                                }
                                tr {
                                    td { "Roof solar ~5 kW + MPPT" }
                                    td { "$1.5k" }
                                    td { "$2.5k" }
                                    td { "$4k" }
                                }
                                tr {
                                    td { "Autonomy hardware (cameras, computer)" }
                                    td { "$0.8k" }
                                    td { "$1.5k" }
                                    td { "$2.5k" }
                                }
                                tr {
                                    td { "Charger, thermal, harnesses, displays" }
                                    td { "$3k" }
                                    td { "$5k" }
                                    td { "$8k" }
                                }
                                tr {
                                    td { "Assembly, paint, end-of-line" }
                                    td { "$6k" }
                                    td { "$10k" }
                                    td { "$18k" }
                                }
                                tr {
                                    td { "Warranty reserve + logistics" }
                                    td { "$2.5k" }
                                    td { "$4k" }
                                    td { "$6k" }
                                }
                                tr class="total-row" {
                                    td { strong { "Unit COGS" } }
                                    td { strong { "~$61k" } }
                                    td { strong { "~$95k" } }
                                    td { strong { "~$151k" } }
                                }
                            }
                        }
                    }

                    h3 { "Sticker price" }
                    p {
                        "Gross margin, R&D amortization, and overhead often put volume EV "
                        "stickers roughly in a " strong { "1.4–1.8×" } " band over manufacturing "
                        "cost (order-of-magnitude, not a finance model). That maps to:"
                    }
                    div class="table-wrap" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "Scenario" }
                                    th { "COGS" }
                                    th { "Implied MSRP" }
                                }
                            }
                            tbody {
                                tr {
                                    td { "Optimistic volume + spartan interior" }
                                    td { "~$65–75k" }
                                    td { strong { "~$100–120k" } }
                                }
                                tr {
                                    td { "Base product (recommended planning number)" }
                                    td { "~$90–105k" }
                                    td { strong { "~$130–170k" } }
                                }
                                tr {
                                    td { "First-gen / low volume / nicer fit-out" }
                                    td { "~$130–160k" }
                                    td { strong { "~$200–280k" } }
                                }
                            }
                        }
                    }
                    p {
                        "Anchors: a Model Y is ~$40–50k; a Tesla Semi is ~$260–300k; mid Class C "
                        "and premium camper vans often land ~$200–260k+; Class A coaches "
                        "frequently $250–500k+. The interesting claim is not “cheaper than a "
                        "Model Y” — body and wet cell make that unrealistic — but "
                        strong { "undercutting traditional Class A money while beating Semi "
                        "money" } ", by deleting cargo GCW, most of the pack, and most of the "
                        "motors."
                    }

                    h3 { "Recommended product sketch" }
                    div class="table-wrap" {
                        table class="data-table" {
                            thead {
                                tr {
                                    th { "Spec" }
                                    th { "Value" }
                                }
                            }
                            tbody {
                                tr {
                                    td { "Envelope" }
                                    td { "40′ HC exterior dimensions; non-ISO structure" }
                                }
                                tr {
                                    td { "GVW class" }
                                    td { "~8 t" }
                                }
                                tr {
                                    td { "Max speed" }
                                    td { "80 km/h (software-limited)" }
                                }
                                tr {
                                    td { "Pack" }
                                    td { "~90 kWh (LFP or equivalent volume chemistry)" }
                                }
                                tr {
                                    td { "Drive" }
                                    td { "1–2 motors, ~200 kW peak total" }
                                }
                                tr {
                                    td { "Range" }
                                    td { "~180–220 km usable — duty-cycle, not highway hero" }
                                }
                                tr {
                                    td { "Solar" }
                                    td { "~5 kW roof; ~10–20 kWh/day typical harvest" }
                                }
                                tr {
                                    td { "Autonomy" }
                                    td { "Camera + compute stack; product truth still “supervised” until regulators say otherwise" }
                                }
                                tr {
                                    td { "Target MSRP" }
                                    td { strong { "$140k ± $30k" } " at scale" }
                                }
                            }
                        }
                    }
                    p {
                        "Where cost still bites even for Tesla: multi-axle motorhome "
                        "certification; RV interior labor; low volume in early years; and "
                        "liability for unsupervised driving with sleeping occupants. Battery "
                        "and motors are no longer the cliffs once the mission is honest."
                    }
                }

                section {
                    h2 { "7. Open questions and a FOSS angle" }
                    p {
                        "The cost model above assumes a closed, high-volume OEM. Fossall’s "
                        "interest is different: which pieces of the same idea can still be "
                        "open even if the vehicle is not a git repo?"
                    }
                    ul {
                        li {
                            strong { "Software" } " — energy management UIs, trip planners "
                            "tuned for RV duty cycles, open diagnostics, non-cloud lock-in "
                            "for vehicle telemetry the owner actually owns."
                        }
                        li {
                            strong { "Design" } " — published dimensions, mass budgets, solar "
                            "and pack assumptions, and interior modules others can fork (the "
                            "way open hardware frames spread in other domains)."
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
                        "adjectives at once in their strongest form — unlimited range, "
                        "unsupervised door-to-door autonomy, and Model Y pricing. Narrow the "
                        "mission to " strong { "short moves, mostly parked, 80 km/h, "
                        "honest solar, ~90 kWh" } " and a Tesla-scale builder could "
                        "plausibly land near " strong { "$130–170k" } ". That is not free. "
                        "It " em { "is" } " in the same conversation as premium vans and mid "
                        "Class A coaches, with a better energy story."
                    }
                }

                section class="closing" {
                    h2 { "Next" }
                    p {
                        "This page is still a sketch, but the cost model is now something "
                        "others can argue with: mass budget, solar yield, pack size, and a "
                        "Tesla-scale BOM. Follow-ons: tighter chassis platform choices, "
                        "interior mass/cost, and regulatory path by market. Until then: FOSS "
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

/// Interactive 3D sketch of the 40′ container-scale EV-RV (Three.js).
fn rv_model_widget() -> Markup {
    html! {
        figure class="rv-model-figure" id="rv-model-figure" {
            figcaption class="rv-model-caption" {
                h3 { "Proposed form: 40′ cabless EV-RV" }
                p {
                    "Procedural sketch — ISO 40′ high-cube envelope on a cabless multi-axle "
                    "electric chassis with autonomy sensor pods. Inspired by self-driving "
                    "hauler form factors; not an engineering CAD model."
                }
            }
            div class="rv-model" id="rv-model" {
                canvas aria-label="Interactive 3D model of a 40-foot container-scale electric self-driving RV" {}
                div class="rv-model-toolbar" role="toolbar" aria-label="Model controls" {
                    button type="button" class="rv-model-btn" data-rv-action="reset" { "Reset view" }
                    button type="button" class="rv-model-btn" data-rv-action="rotate" aria-pressed="true" { "Auto-rotate" }
                    button type="button" class="rv-model-btn" data-rv-action="cutaway" aria-pressed="false" { "Cutaway" }
                    button type="button" class="rv-model-btn" data-rv-action="chassis" aria-pressed="false" { "Chassis only" }
                    button type="button" class="rv-model-btn" data-rv-action="labels" aria-pressed="true" { "Labels" }
                }
                p class="rv-model-status" data-rv-status { "Loading 3D viewer…" }
            }
            (PreEscaped(RV_MODEL_BOOTSTRAP))
        }
    }
}

/// Mount the procedural Three.js model (import map lives in layout head).
/// Inline module so HTMX boost re-evaluates on navigation to `/rv`.
const RV_MODEL_BOOTSTRAP: &str = r#"
<script type="module">
  import { mountRvModel } from '/static/js/rv-model.js';
  const host = document.getElementById('rv-model');
  if (host && host.dataset.mounted !== '1') {
    host.dataset.mounted = '1';
    mountRvModel(host);
  }
</script>
"#;

/// Interactive pack estimator shell; logic runs in `/wasm/fossall_wasm.js`.
fn pack_estimator_widget() -> Markup {
    html! {
        aside class="widget" id="pack-estimator" {
            h3 { "Rough pack estimator" }
            p class="widget-note" {
                "Client-side Rust (WASM). Order-of-magnitude only — ~160 Wh/kg pack, "
                "~$100/kWh volume planning band, ~450 Wh/km for a tall boxy RV. "
                "Default ~90 kWh is the short-range / mostly-parked product case."
            }
            label for="pack-kwh" {
                "Pack size: "
                strong id="pack-kwh-label" { "90" }
                " kWh"
            }
            input type="range" id="pack-kwh" min="20" max="400" step="10" value="90"
                aria-valuemin="20" aria-valuemax="400" aria-valuenow="90";
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
