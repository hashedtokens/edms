# ── Stage 1: Build edms-child from compute/ ─────────────────────────
FROM rust:1.90-bookworm AS compute-builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /compute
COPY compute/Cargo.toml compute/Cargo.lock ./
COPY compute/src ./src
# compute/Cargo.toml depends on edms via a relative path
# (../webserver/libs/edms), so that crate has to exist in the build
# context too, at the same relative location, before `cargo build` runs.
COPY webserver/libs /webserver/libs
RUN cargo build --release

# ── Stage 2: Build rust-webserver from webserver/ ───────────────────
FROM rust:1.90-bookworm AS webserver-builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /webserver
COPY compute/Cargo.toml compute/Cargo.lock /compute/
COPY compute/src /compute/src
COPY webserver/Cargo.toml webserver/Cargo.lock ./
COPY webserver/libs ./libs
COPY webserver/src ./src
RUN cargo build --release

# ── Stage 3: Runtime ────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    libssl3 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Webserver binary
COPY --from=webserver-builder /webserver/target/release/rust-webserver .

# edms-child — placed where ipc.rs looks for it
COPY --from=compute-builder /compute/target/release/edms-child ./target/release/edms-child

# Was `cp webserver/src/config.yml ./src/config.yml || true` — that file
# never existed (the real one is webserver/config.yaml, no src/), so this
# silently no-op'd on every build and the app always ran on
# AppConfig::default() in Docker, never the real config. EDMS_CONFIG_PATH
# defaults to "config.yaml" relative to CWD (/app here), so this is where
# it actually needs to land.
COPY webserver/config.yaml ./config.yaml

RUN mkdir -p edms_root/endpoints/reports \
             edms_root/exports \
             edms_root/session-backup \
             edms_root/active \
             edms_root/temp \
             edms_root/repo \
             edms_data

RUN mkdir -p /edms_root

# Runs as root deliberately. This image is a locally-run dev/eval tool
# distributed for people to `docker compose up` on any machine — with a
# non-root user, a bind-mounted host directory (./webserver/data) stays
# owned by whatever UID the host created it as, and the container user
# can't write to it, so SQLite fails with CannotOpen. That broke the
# team's runs on Linux (issue #31). Root sidesteps the UID-mismatch
# entirely; proper privilege-drop is a separate hardening task.

EXPOSE 3000
CMD ["./rust-webserver"]