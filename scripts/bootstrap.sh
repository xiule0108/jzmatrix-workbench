#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

node_version="24.14.0"
npm_version="11.9.0"
rust_version="1.85.1"
rustup_url="https://sh.rustup.rs"

cargo_bin_dir="${CARGO_HOME:-${HOME}/.cargo}/bin"
if [[ -x "$cargo_bin_dir/rustup" ]]; then
  PATH="$cargo_bin_dir:$PATH"
  export PATH
fi

fail_toolchain() {
  printf 'bootstrap blocked: %s\n' "$1" >&2
  exit 2
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  fail_toolchain "this bootstrap entry is for macOS; use CI or the Windows entry on Windows"
fi

command -v node >/dev/null 2>&1 || fail_toolchain "Node.js ${node_version} is required"
command -v npm >/dev/null 2>&1 || fail_toolchain "npm ${npm_version} is required"
[[ "$(node --version)" == "v${node_version}" ]] || fail_toolchain "expected Node.js ${node_version}, found $(node --version)"
[[ "$(npm --version)" == "${npm_version}" ]] || fail_toolchain "expected npm ${npm_version}, found $(npm --version)"

if ! command -v rustup >/dev/null 2>&1; then
  if [[ "${1:-}" != "--install-rust" ]]; then
    fail_toolchain "rustup is missing; rerun with --install-rust to install the pinned user-level toolchain"
  fi
  command -v curl >/dev/null 2>&1 || fail_toolchain "curl is required for the official rustup bootstrap"
  temp_dir="$(mktemp -d)"
  trap 'rm -rf "$temp_dir"' EXIT
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location "$rustup_url" --output "$temp_dir/rustup-init.sh"
  sh "$temp_dir/rustup-init.sh" -y --profile minimal --default-toolchain "$rust_version"
fi

if ! command -v rustup >/dev/null 2>&1 && [[ -x "$cargo_bin_dir/rustup" ]]; then
  PATH="$cargo_bin_dir:$PATH"
  export PATH
fi

command -v rustup >/dev/null 2>&1 || fail_toolchain "rustup installation did not provide a usable command"
rustup toolchain install "$rust_version" \
  --profile minimal \
  --component rustfmt \
  --component clippy \
  --target aarch64-apple-darwin \
  --target x86_64-apple-darwin \
  --target x86_64-pc-windows-msvc

rustup run "$rust_version" rustc --version | grep -F "rustc ${rust_version} " >/dev/null \
  || fail_toolchain "pinned Rust toolchain validation failed"

npm ci
npm run lint
npm run typecheck
npm run build
cargo fmt --check
cargo test --workspace --locked
cargo check --workspace --locked
cargo build --locked -p jzmatrix-cli

printf 'bootstrap passed: Node.js %s, npm %s, Rust %s\n' "$node_version" "$npm_version" "$rust_version"
