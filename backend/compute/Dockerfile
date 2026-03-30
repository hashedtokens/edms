# --- Builder Stage ---
FROM rust:1.88-slim AS builder

# 1. Install headers here so the linker can find them
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    curl \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*



WORKDIR /app

COPY Cargo.toml Cargo.lock ./

# Cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Build actual source
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release

# --- Runtime Stage ---
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/endpoint_mng_sys .
COPY src/config.yml ./src/config.yml

RUN mkdir -p edms_root/endpoints/reports \
    edms_root/docs/inbox \
    edms_root/exports \
    edms_root/docs/archive \
    edms_root/static_site

CMD ["./endpoint_mng_sys"]