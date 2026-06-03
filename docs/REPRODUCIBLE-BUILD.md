# Reproducible builds

`blindjoin-linux-amd64.tar.gz` rebuilds byte-for-byte from source on the pinned `ubuntu-24.04` runner image. Verify any release tarball against the recipe below.

## Recipe A — native (exact byte match on the CI image)

Run on a fresh `ubuntu-24.04` shell that matches the CI runner — a fresh `ubuntu-24.04` GitHub Actions runner, a `docker run --rm -it ubuntu:24.04` shell, or an `ubuntu-24.04` cloud VM. Toolchain pin is resolved by `rust-toolchain.toml`.

```bash
git clone https://github.com/<owner>/blindjoin.git
cd blindjoin
git checkout vX.Y.Z

export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)
export RUSTFLAGS="--remap-path-prefix=$(pwd)=/build --remap-path-prefix=$HOME/.cargo=/cargo"
export CARGO_INCREMENTAL=0

cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot

mkdir -p dist
cp target/release/coordinator target/release/client target/release/liquidity-bot dist/
tar --sort=name --owner=0 --group=0 --numeric-owner \
    --mtime="@${SOURCE_DATE_EPOCH}" \
    -cf - -C dist . \
  | gzip -n > blindjoin-linux-amd64.tar.gz

sha256sum blindjoin-linux-amd64.tar.gz
```

The output `sha256sum` must match the value below for the tag you checked out.

## Recipe B — verify on any machine using Docker

Use this on macOS / Arch / NixOS / Windows-WSL / any host that isn't `ubuntu-24.04` natively. Runs the same recipe inside a fresh `ubuntu:24.04` container. Expected outcome: byte-equal output to Recipe A in most environments — but Docker Hub's `ubuntu:24.04` and the GitHub Actions `ubuntu-24.04` runner image diverge slightly in glibc / linker versions, so a sha256 mismatch here can be a benign distro-layer difference rather than tampering. If you want a hard byte-equality guarantee, use Recipe A.

```bash
git clone https://github.com/<owner>/blindjoin.git
cd blindjoin
git checkout vX.Y.Z

docker run --rm -v "$(pwd)":/work -w /work ubuntu:24.04 bash -euxc '
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y -qq curl git build-essential pkg-config libssl-dev libsqlite3-dev ca-certificates
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain none --profile minimal
  . "$HOME/.cargo/env"

  export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)
  export RUSTFLAGS="--remap-path-prefix=/work=/build --remap-path-prefix=$HOME/.cargo=/cargo"
  export CARGO_INCREMENTAL=0

  cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot

  mkdir -p dist
  cp target/release/coordinator target/release/client target/release/liquidity-bot dist/
  tar --sort=name --owner=0 --group=0 --numeric-owner \
      --mtime="@${SOURCE_DATE_EPOCH}" \
      -cf - -C dist . \
    | gzip -n > blindjoin-linux-amd64.tar.gz

  sha256sum blindjoin-linux-amd64.tar.gz
'
```

`rust-toolchain.toml` inside the mounted source drives rustup to install `1.95.0` (`profile = "minimal"` + rustfmt + clippy) on the first `cargo` invocation. No host-side Rust install is needed.

## Expected sha256sum

| Tag | sha256 |
| --- | --- |
| v1.6.0 | `3dd0679fd7d1135aefabc99e242df77c0a6af903c65cd3713d24b5e4d3ce6fd6` |

Replaced at the v1.6.0 cut by running `.github/workflows/reproducible-verify.yml` via `workflow_dispatch` and copying the `Rebuilt locally:` line from its log into this table and into the workflow's `EXPECTED_SHA256` env.

## Toolchain

- rustc / cargo `1.95.0` — pinned in [`rust-toolchain.toml`](../rust-toolchain.toml) and in 6 `with: toolchain:` blocks across `release.yml` + `ci.yml`. Bump all together.
- ubuntu-24.04 runner image — pinned in `release.yml` build job + `reproducible-verify.yml`.
- `RUSTFLAGS` strips embedded build-host paths; `CARGO_INCREMENTAL=0` disables incremental compilation; `SOURCE_DATE_EPOCH` is the tagged commit's committer time; `tar` flags + `gzip -n` strip filesystem and gzip-header noise.

Re-prove reproducibility at any time by dispatching `.github/workflows/reproducible-verify.yml` from the Actions tab.
