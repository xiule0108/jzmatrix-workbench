#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v cargo >/dev/null 2>&1; then
  printf '%s\n' '{"contract":"jzmatrix.cli-response","version":"1.0.0","command":"doctor","ok":false,"status":"blocked","outcome":"not_committed","errors":[{"code":"rust_toolchain_missing","message":"Rust toolchain is not installed","retryable":false,"outcome":"not_committed","details_ref":null}]}'
  exit 2
fi

exec cargo run --offline --locked -p jzmatrix-cli -- doctor --json
