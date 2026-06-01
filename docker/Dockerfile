# docker/Dockerfile
# Single multi-stage build for all binaries using cargo-chef for layer caching.
#
# Supply-chain pinning (P0-2/3, v1.5 release-readiness):
#   The two base image references below are ARG-overridable so a release build
#   can pin to a digest without editing the Dockerfile. The defaults are
#   floating tags for developer ergonomics; tagged-release CI MUST override
#   both ARGs with a digest-form reference.
#
#   Release procedure (until cosign attestations land in v1.6):
#     1. On a clean runner, run:
#          docker pull debian:bookworm-slim
#          docker inspect --format='{{index .RepoDigests 0}}' debian:bookworm-slim
#          docker pull lukemathwalker/cargo-chef:latest-rust-1
#          docker inspect --format='{{index .RepoDigests 0}}' lukemathwalker/cargo-chef:latest-rust-1
#     2. Record the resulting `image@sha256:HEX` strings.
#     3. Pass them to `docker build`:
#          docker build \
#            --build-arg DEBIAN_REF=debian@sha256:<HEX> \
#            --build-arg CARGO_CHEF_REF=lukemathwalker/cargo-chef@sha256:<HEX> \
#            -f docker/Dockerfile -t blindjoin-coordinator:1.5.0 .
#     4. The published image digest is then a function of the build inputs +
#        the pinned base digests; an independent rebuilder using the same
#        ARGs on a clean toolchain produces byte-identical layers down to
#        the cargo-chef stage.
#
#   The v1.6 supply-chain milestone (see SECURITY.md § Supply-chain status)
#   will: (a) automate the digest refresh + drift check via CI; (b) emit
#   cosign attestations on the published image; (c) publish reproducible-
#   build instructions so anyone can verify the digest matches source.

ARG CARGO_CHEF_REF=lukemathwalker/cargo-chef:latest-rust-1
ARG DEBIAN_REF=debian:bookworm-slim

FROM ${CARGO_CHEF_REF} AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin coordinator --bin client --bin liquidity-bot

FROM ${DEBIAN_REF} AS runtime-base
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
