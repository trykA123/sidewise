#!/usr/bin/env bash
# Puts the pinned Inno Setup into the Windows VM (plan P4, atom IN1, NC175).
# Downloads once on the host, checks the hash, then runs vm-inno.ps1 in the VM under the VM lock.
set -euo pipefail

readonly INNO_VERSION='7.1.1'
readonly INNO_FILE="innosetup-${INNO_VERSION}-x64.exe"
readonly INNO_URL='https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x64.exe'
readonly INNO_SHA256='8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0'
readonly LOCK_WAIT_S=3600
readonly REMOTE_DIR='C:/winvm'

tmp=''

# Prints the usage line to stderr.
usage() {
  printf '%s\n' "usage: WINVM=<path to winvm.sh> vm-inno.sh" >&2
}

# Returns 0 when file $1 has the pinned SHA-256.
hash_ok() {
  printf '%s  %s\n' "$INNO_SHA256" "$1" | sha256sum -c --quiet >/dev/null 2>&1
}

# Makes sure the cached installer exists and matches the pin; prints its path. Dies on a mismatch.
fetch() {
  local dir="$1/downloads" file
  file="$dir/$INNO_FILE"
  mkdir -p "$dir"
  if [[ -f "$file" ]] && hash_ok "$file"; then
    printf '%s\n' "$file"
    return 0
  fi
  tmp="$(mktemp -p "$dir" inno.XXXXXX)"
  curl -fsSL --proto '=https' --max-time 300 -o "$tmp" -- "$INNO_URL"
  if ! hash_ok "$tmp"; then
    rm -f -- "$tmp"
    printf '%s\n' "vm-inno: hash mismatch for $INNO_FILE" >&2
    exit 1
  fi
  mv -- "$tmp" "$file"
  printf '%s\n' "$file"
}

# Installs Inno Setup in the VM when absent; fails when the VM stays busy or the install is not ready.
main() {
  if [[ "${1:-}" == "-h" ]]; then usage; exit 0; fi
  if [[ $# -gt 0 ]]; then usage; exit 2; fi
  command -v curl >/dev/null || { printf '%s\n' "vm-inno: curl missing" >&2; exit 1; }
  local winvm="${WINVM:?set WINVM to the path of winvm.sh}"
  local logs="${WINVM_LOGS:-$HOME/.cache/winvm-ci}"
  local script_dir installer log
  script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
  mkdir -p "$logs"
  trap 'rm -f -- "$tmp"' EXIT
  installer="$(fetch "$logs")"

  exec 9>"$logs/.vm.lock"
  if ! flock -w "$LOCK_WAIT_S" 9; then
    printf '%s\n' "vm-inno: the VM stayed busy for ${LOCK_WAIT_S} s" >&2
    exit 75
  fi

  "$winvm" scp -q "$installer" "admin@127.0.0.1:$REMOTE_DIR/$INNO_FILE"
  "$winvm" scp -q "$script_dir/vm-inno.ps1" "admin@127.0.0.1:$REMOTE_DIR/sidewise-inno.ps1"
  log="$logs/sidewise-inno.log"
  printf '%s\n' "log: $log" >&2
  local status=0
  "$winvm" ssh "powershell -NoProfile -ExecutionPolicy Bypass -File C:\\winvm\\sidewise-inno.ps1 -Installer C:\\winvm\\$INNO_FILE -Sha256 $INNO_SHA256 -Version $INNO_VERSION" \
    > "$log" 2>&1 9>&- || status=$?
  grep -E '^=== ' "$log" || true
  if [[ "$status" -ne 0 ]] || ! grep -q '^=== inno ready' "$log"; then
    printf '%s\n' "vm-inno: failed (ssh exit $status); read $log" >&2
    exit 1
  fi
}

main "$@"
