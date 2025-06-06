ARG ACCEL_PEN_CONFIG_FILE="./accel_pen.toml"

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
COPY . .
RUN cargo chef cook --release --recipe-path recipe.json
RUN cargo build --bin accel_pen

FROM debian:bookworm AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/accel_pen .
COPY . .
ENTRYPOINT ["/app/accel_pen", "/app/accel_pen.toml"]
