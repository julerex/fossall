# Fossall

**FOSS ALL THE THINGS.**

A small public site built almost entirely in Rust:

- **Axum** server, HTML via **maud**
- **HTMX** for navigation (vendored, no SPA framework)
- **Rust WASM** for the interactive battery-pack estimator on the essay page
- **sqlx** + Fly.io Managed Postgres for the five-letter word list
- Deployed on **Fly.io** at [fossall.com](https://fossall.com)

Content:

- `/rv` — feasibility sketch of a low-cost, fully electric, fully self-driving recreational vehicle about the size of a shipping container (interactive Three.js model, solar yield math, Tesla-scale cost estimate, and a comparison to U.S. house prices / apartment rents / Tesla-style leases)
- `/homeprices` — why U.S. home prices are still high in July 2026, with a 30-year look at land vs building materials vs labor
- `/words` — every five-letter string for a two-letter start; ENABLE words (public domain) highlighted from Postgres

## Local development

Requirements: Rust stable, `wasm32-unknown-unknown` target, `wasm-bindgen-cli`.

```bash
# Install wasm target + bindgen if needed
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.120

make dev
# → http://localhost:8080/
# → http://localhost:8080/rv
# → http://localhost:8080/homeprices
# → http://localhost:8080/words   (503 unless DATABASE_URL is set)
```

`/words` needs the Fly MPG proxy and `DATABASE_URL`. Other pages work without it. Setup, seed, and connection details: [docs/DATABASE.md](docs/DATABASE.md).

Other targets:

```bash
make build-wasm   # client WASM → static/wasm/
make build        # release server + wasm
make test
make lint
make fmt
make seed         # insert data/five_letter_words.txt (requires DATABASE_URL)
```

## Deploy (Fly.io)

```bash
fly auth login
fly apps create fossall   # once
fly deploy
```

Optional CI: push to `main` with GitHub secret `FLY_API_TOKEN` (see `.github/workflows/deploy.yml`).

```bash
make fly-logs
```

## Custom domain

`fossall.com` is managed in Cloudflare. Point DNS at the Fly app and issue certs as described in [docs/DOMAIN_SETUP.md](docs/DOMAIN_SETUP.md).

## Layout

```
server/         Axum + maud pages
client-wasm/    wasm-bindgen pack estimator
static/         CSS, htmx, favicon, generated wasm/
data/           ENABLE five-letter word list
db/             Postgres schema
Dockerfile      multi-stage build
fly.toml        Fly app config
```

## License

MIT — see [LICENSE](LICENSE).
