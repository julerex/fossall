# Build client WASM
FROM rust:1.92 AS wasm-builder

WORKDIR /app
RUN rustup target add wasm32-unknown-unknown
# Must match client-wasm's wasm-bindgen crate version in Cargo.lock.
RUN cargo install wasm-bindgen-cli --version 0.2.120

COPY Cargo.toml Cargo.lock ./
COPY client-wasm ./client-wasm
COPY server/Cargo.toml ./server/Cargo.toml
RUN mkdir -p server/src && echo "fn main() {}" > server/src/main.rs

RUN cargo build -p fossall-wasm --target wasm32-unknown-unknown --release --locked

RUN mkdir -p /out/wasm && \
    wasm-bindgen --no-typescript --target web \
        --out-dir /out/wasm \
        --out-name fossall_wasm \
        /app/target/wasm32-unknown-unknown/release/fossall_wasm.wasm

# Build Axum server
FROM rust:1.92 AS server-builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY server ./server
COPY client-wasm ./client-wasm

# Dependency cache layer
RUN mkdir -p server/src client-wasm/src && \
    echo "fn main() {}" > server/src/main.rs && \
    echo "" > client-wasm/src/lib.rs
RUN cargo build -p fossall-server --release --locked || true

COPY server ./server
COPY client-wasm ./client-wasm
RUN touch server/src/main.rs client-wasm/src/lib.rs && \
    cargo build -p fossall-server --release --locked

# Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=server-builder /app/target/release/fossall-server /app/server
COPY static /app/static
COPY --from=wasm-builder /out/wasm /app/static/wasm

ENV PORT=8080
EXPOSE 8080

CMD ["/app/server"]
