#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"

export PATH="$HOME/.cargo/bin:$PATH"

if command -v rustup >/dev/null 2>&1; then
    rustup target add wasm32-unknown-unknown >/dev/null
fi

if ! command -v candid-extractor >/dev/null 2>&1; then
    echo "candid-extractor is required. Install it with: cargo install candid-extractor --locked" >&2
    exit 1
fi

CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="${CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER:-gcc}" \
    cargo build --target wasm32-unknown-unknown --release -p domm-degens-canister

mkdir -p target/dfx/degens
candid-extractor target/wasm32-unknown-unknown/release/domm_degens_canister.wasm \
    > target/dfx/degens/degens.did
