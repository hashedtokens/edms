# ── Stage 1: Builder ────────────────────────────────────────────────
FROM rust:1.85 AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

# Cache deps — stub out all entry points
RUN mkdir -p src/bin \
    && echo "fn main() {}" > src/main.rs \
    && echo "fn main() {}" > src/bin/child.rs \
    && echo "" > src/lib.rs
RUN cargo build --release
RUN rm -rf src

# Build real source
COPY src ./src
RUN touch src/main.rs src/bin/child.rs src/lib.rs
RUN cargo build --release

# ── Stage 2: Runtime ────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    libssl3 \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Both binaries — compute server for dev, edms-child for production IPC
COPY --from=builder /app/target/release/compute .
COPY --from=builder /app/target/release/edms-child ./target/release/edms-child

COPY src/config.yml ./src/config.yml

RUN mkdir -p edms_root/endpoints/reports \
             edms_root/exports \
             edms_root/session-backup \
             edms_root/active \
             edms_root/temp \
             edms_root/repo

RUN useradd -m appuser && chown -R appuser /app
USER appuser

EXPOSE 3000
CMD ["./compute"]