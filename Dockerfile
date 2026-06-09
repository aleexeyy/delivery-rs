# create a recipe for dependency caching
FROM lukemathwalker/cargo-chef:0.1.77-rust-1.95.0 AS chef
WORKDIR /app

FROM chef AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# build the app
FROM chef AS builder
WORKDIR /app
ENV SQLX_OFFLINE=true

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --locked --bin delivery-rs

# run the app
FROM debian:bookworm-slim AS runtime

RUN useradd -r -u 100 -s /bin/false -M appuser
WORKDIR /app

COPY --from=curlimages/curl:latest /usr/bin/curl /usr/bin/curl
COPY --from=builder --chown=appuser:appuser /app/target/release/delivery-rs .

USER appuser

ARG RUST_LOG="delivery-rs=debug,tower_http=debug"
ENV RUST_LOG=${RUST_LOG} \
    HOST="0.0.0.0" \
    APP_PORT="3000" \
    DATABASE_URL="fail" \
    RUN_MIGRATIONS="true"

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=3s \
  CMD curl -f http://localhost:3000/health || exit 1

CMD ["./delivery-rs"]
