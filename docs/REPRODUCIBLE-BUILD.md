# Reproducible builds

`blindjoin-linux-amd64.tar.gz` rebuilds byte-for-byte from source on the pinned `ubuntu-24.04` runner image. Verify any release tarball against the recipe below.

## Recipe

Run on a fresh `ubuntu-24.04` shell (Docker, VM, or GitHub Actions runner). Toolchain pin is resolved by `rust-toolchain.toml`.

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
