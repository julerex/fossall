# Fossall

**FOSS ALL THE THINGS.**

A small public site built almost entirely in Rust:

- **Axum** server, HTML via **maud**
- **HTMX** for navigation (vendored, no SPA framework)
- **Rust WASM** for the interactive battery-pack estimator on the essay page
- Deployed on **Fly.io** at [fossall.com](https://fossall.com)

First piece of content: a feasibility sketch of a low-cost, fully electric, fully self-driving recreational vehicle about the size of a shipping container — see `/rv` (includes an interactive Three.js model of a 40′ cabless EV-RV form factor).

## Local development

Requirements: Rust stable, `wasm32-unknown-unknown` target, `wasm-bindgen-cli`.

```bash
# Install wasm target + bindgen if needed
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.120

make dev
# → http://localhost:8080/
# → http://localhost:8080/rv
```

Other targets:

```bash
make build-wasm   # client WASM → static/wasm/
make build        # release server + wasm
make test
make lint
make fmt
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
Dockerfile      multi-stage build
fly.toml        Fly app config
```

## License

MIT — see [LICENSE](LICENSE).
