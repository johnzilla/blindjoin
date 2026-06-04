# docker/Dockerfile
# Single multi-stage build for all binaries using cargo-chef for layer caching.
#
# Base images are pinned by sha256 digest below. To bump:
#   docker buildx imagetools inspect debian:bookworm-slim                    --format '{{.Manifest.Digest}}'
#   docker buildx imagetools inspect lukemathwalker/cargo-chef:latest-rust-1 --format '{{.Manifest.Digest}}'
# Replace the digest portion of the matching FROM line below with the new
# sha256:HEX value. One bump per PR; see CONTRIBUTING.md §Bumping base-image
# digests.

FROM lukemathwalker/cargo-chef:latest-rust-1@sha256:e606721f52d95169364bf39cae726a94ed8b397625011ccfaa8340db488b823b AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin coordinator --bin client --bin liquidity-bot

FROM debian:bookworm-slim@sha256:0104b334637a5f19aa9c983a91b54c89887c0984081f2068983107a6f6c21eeb AS runtime-base
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

FROM runtime-base AS coordinator
WORKDIR /app
RUN mkdir -p /app/keys /app/data
COPY --from=builder /app/target/release/coordinator /usr/local/bin/coordinator
ENTRYPOINT ["/usr/local/bin/coordinator"]

FROM runtime-base AS client
WORKDIR /app
COPY --from=builder /app/target/release/client /usr/local/bin/client
ENTRYPOINT ["/usr/local/bin/client"]

FROM runtime-base AS liquidity-bot
WORKDIR /app
# Create /app/data directory for the rotation counter file (BLINDJOIN_BOT_COUNTER_FILE).
# The Docker named volume `bot-data` overlays this directory at runtime, but the mkdir
# ensures the mount-point exists when Docker does not auto-create it (Pitfall 6).
RUN mkdir -p /app/data
COPY --from=builder /app/target/release/liquidity-bot /usr/local/bin/liquidity-bot
ENTRYPOINT ["/usr/local/bin/liquidity-bot"]
