#!/usr/bin/env bash
# The quality gate of this repo: format, lints, tests, cargo-deny and lock drift against the
# navo rev in scripts/navo-rev. Run it before every merge to main; the first failing step stops it.
set -euo pipefail

usage() {
  printf '%s\n' "usage: gate.sh   (NAVO_LOCK=<navo's Cargo.lock at the pin> overrides the lookup)" >&2
}

# Prints the path of navo's lockfile at the pin: NAVO_LOCK, else a cached download of the file at
# the rev in scripts/navo-rev (the rev of the vendored fork; bump both together).
navo_lock() {
  if [[ -n "${NAVO_LOCK:-}" ]]; then
    printf '%s\n' "$NAVO_LOCK"
    return
  fi
  local rev dir file tmp
  rev="$(cat -- scripts/navo-rev 2>/dev/null)" || rev=""
  if [[ ! "$rev" =~ ^[0-9a-f]{40}$ ]]; then
    printf '%s\n' "gate: scripts/navo-rev is missing or not 40 hex characters; set NAVO_LOCK" >&2
    return 1
  fi
  dir="target/navo-lock"
  file="$dir/$rev.Cargo.lock"
  if [[ ! -s "$file" ]]; then
    mkdir -p -- "$dir"
    tmp="$(mktemp -- "$dir/.dl.XXXXXX")"
    if ! curl -fsSL --max-time 60 -o "$tmp" "https://raw.githubusercontent.com/trykA123/navo/$rev/Cargo.lock"; then
      rm -f -- "$tmp"
      printf '%s\n' "gate: cannot fetch navo's Cargo.lock at $rev; set NAVO_LOCK" >&2
      return 1
    fi
    mv -- "$tmp" "$file"
  fi
  printf '%s\n' "$file"
}

# Runs one named step and prints its result; a failing step ends the gate with exit code 1.
step() {
  local name="$1"
  shift
  printf '%s\n' "=== $name" >&2
  if ! "$@"; then
    printf '%s\n' "=== FAIL $name" >&2
    exit 1
  fi
}

main() {
  if [[ "${1:-}" == "-h" ]]; then usage; exit 0; fi
  if [[ $# -gt 0 ]]; then usage; exit 2; fi
  command -v cargo-deny >/dev/null || { printf '%s\n' "cargo-deny missing" >&2; exit 1; }
  local script_dir other
  script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
  cd -- "$script_dir/.."
  other="$(navo_lock)"
  step fmt cargo fmt --all --check
  step clippy cargo clippy --workspace --all-targets --locked -- -D warnings
  step test cargo test --workspace --locked
  step deny cargo deny --locked check
  step lock-drift cargo xtask lock-drift --other "$other"
  printf '%s\n' "=== ALL GREEN" >&2
}

main "$@"
