# AGENTS.md

Public Rust site at [fossall.com](https://fossall.com). Product and content: [README.md](README.md). Domain/DNS: [docs/DOMAIN_SETUP.md](docs/DOMAIN_SETUP.md).

## Stack

Cargo workspace (`resolver = "2"`): `server` (Axum + maud + sqlx) and `client-wasm` (wasm-bindgen). HTMX is vendored at `static/htmx.min.js` — no SPA framework.

`static/wasm/` is gitignored. Local runs need `make build-wasm` first (`make dev` does this). **wasm-bindgen-cli 0.2.120** must match the crate in `Cargo.lock`; the Dockerfile pins the same version.

## Commands

Use the Makefile. It runs cargo as `env -u ARGV0 cargo` so Cursor/agent `ARGV0` proxy errors do not break build/test/clippy.

| Target | What |
|--------|------|
| `make dev` | WASM + `cargo run -p fossall-server` → http://localhost:8080/ |
| `make test` | `fossall-server` and `fossall-wasm` tests |
| `make lint` | `fmt --check` + clippy `-D warnings` on both packages |
| `make fmt` | `cargo fmt --all` |
| `make build` | release server + wasm |
| `make seed` | insert `data/five_letter_words.txt` (needs `DATABASE_URL`) |
| `make seed-earth` | ICS + Macrostrat + PBDB + GPlates into database `earth` (`EARTH_DATABASE_URL`) |
| `make earth-mpg-setup` | Create `earth` DB/user on postgreputest, apply schema, attach `EARTH_DATABASE_URL` |
| `make deploy` | `fly deploy` |
| `make fly-logs` | `fly logs --app fossall` |

## Fly.io (`fossall`)

- Region **iad**. Two Machines: `shared-cpu-1x` / 256 MB. `auto_stop` / `auto_start`, **`min_machines_running = 0`**. That is why this app costs cents; do not raise the minimum or keep a machine started without saying so.
- Shared IPv4 + dedicated IPv6 already allocated. **Do not** `fly ips allocate-v4` without `--shared` ($2/mo dedicated). No volumes. Postgres is the existing MPG cluster **postgreputest** (fra): database `fossall` / schema `words`, and database `earth` / schema `earth`. **Do not** create a new cluster. Attach earth with `--variable-name EARTH_DATABASE_URL` so words `DATABASE_URL` stays intact (`make earth-mpg-setup`). See [docs/DATABASE.md](docs/DATABASE.md) and [docs/EARTH.md](docs/EARTH.md). Never paste `fly mpg status --json`.
- Certs already issued for `fossall.com` and `www.fossall.com`. Cloudflare DNS must stay **grey-cloud** (DNS only) so Fly can terminate TLS; see `docs/DOMAIN_SETUP.md`.
- Deploy: `fly deploy`, or push to `main` with GitHub secret `FLY_API_TOKEN` (`.github/workflows/deploy.yml`).
- `fly status` / `fly certs` can **auto-start** a stopped Machine. It will stop again; still avoid probing in a loop.

## Conventions

- Pages live in `server/src/pages.rs`, `server/src/words.rs`, `server/src/earth.rs`, and `server/src/layout.rs`. Match existing maud + HTMX patterns; do not add a JS framework. `/earth` uses the same CDN Three.js import map as `/rv`. `/health` must not query Postgres.
- After `client-wasm` changes, rebuild WASM before claiming the UI works.
- Run `make lint` and `make test` before finishing code changes.
