# ---------- build stage ----------
FROM rust:1.82 as builder

WORKDIR /usr/src/app

# Create a dummy project to cache dependencies
RUN USER=root cargo new rust-webserver
WORKDIR /usr/src/app/rust-webserver

# Copy manifests & fetch deps
COPY Cargo.toml Cargo.lock* ./
RUN cargo fetch

# Copy source
COPY src ./src
COPY config ./config

# Build in release mode
RUN cargo build --release

# ---------- runtime stage ----------
FROM debian:bookworm-slim

# Create non-root user
RUN useradd -m appuser

WORKDIR /app

# Copy binary & config
COPY --from=builder /usr/src/app/rust-webserver/target/release/rust-webserver /app/rust-webserver
COPY --from=builder /usr/src/app/rust-webserver/config /app/config

ENV CONFIG_PATH=/app/config/config.yaml

USER appuser

EXPOSE 8080

CMD ["./rust-webserver"]
