.PHONY: dev build build-wasm run lint fmt test deploy fly-logs

CARGO = env -u ARGV0 cargo
WASM_BINDGEN ?= wasm-bindgen
WASM_OUT = static/wasm

build-wasm:
	rustup target add wasm32-unknown-unknown
	$(CARGO) build -p fossall-wasm --target wasm32-unknown-unknown --release
	mkdir -p $(WASM_OUT)
	$(WASM_BINDGEN) --no-typescript --target web \
		--out-dir $(WASM_OUT) \
		--out-name fossall_wasm \
		target/wasm32-unknown-unknown/release/fossall_wasm.wasm

dev: build-wasm
	$(CARGO) run -p fossall-server

run: dev

build: build-wasm
	$(CARGO) build -p fossall-server --release

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy -p fossall-server -p fossall-wasm -- -D warnings

fmt:
	$(CARGO) fmt --all

test:
	$(CARGO) test -p fossall-server
	$(CARGO) test -p fossall-wasm

deploy:
	fly deploy

fly-logs:
	fly logs --app fossall
