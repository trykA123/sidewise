#!/usr/bin/env bash
# Runs the silent install, launch, uninstall and leftover check of the Sidewise setup in the Windows VM
# (plan P4, atom IN4, NC177). Copies the setup and vm-install-check.ps1 over, under the VM lock.
set -euo pipefail

readonly LOCK_WAIT_S=3600
readonly REMOTE_DIR='C:/winvm'

# Prints the usage line to stderr.
usage() {
  printf '%s\n' "usage: WINVM=<path to winvm.sh> vm-install-check.sh <setup.exe> <version>" >&2
}

# Checks the setup of $1 with the version $2 in the VM; fails unless the log has the GREEN line.
main() {
  if [[ "${1:-}" == "-h" ]]; then usage; exit 0; fi
  if [[ $# -ne 2 ]]; then usage; exit 2; fi
  local setup="$1" version="$2"
  [[ -s "$setup" ]] || { printf '%s\n' "vm-install-check: no setup at $setup" >&2; exit 1; }
  [[ "$version" =~ ^[0-9A-Za-z.+-]+$ ]] || { printf '%s\n' "vm-install-check: bad version" >&2; exit 2; }
  local winvm="${WINVM:?set WINVM to the path of winvm.sh}"
  local logs="${WINVM_LOGS:-$HOME/.cache/winvm-ci}"
  local script_dir log status=0
  script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
  mkdir -p "$logs"

  exec 9>"$logs/.vm.lock"
  if ! flock -w "$LOCK_WAIT_S" 9; then
    printf '%s\n' "vm-install-check: the VM stayed busy for ${LOCK_WAIT_S} s" >&2
    exit 75
  fi

  "$winvm" scp -q "$setup" "admin@127.0.0.1:$REMOTE_DIR/sw-check-setup.exe"
  "$winvm" scp -q "$script_dir/vm-install-check.ps1" "admin@127.0.0.1:$REMOTE_DIR/sidewise-install-check.ps1"
  log="$logs/sidewise-install-check-$version.log"
  printf '%s\n' "log: $log" >&2
  "$winvm" ssh "powershell -NoProfile -ExecutionPolicy Bypass -File C:\\winvm\\sidewise-install-check.ps1 -Setup C:\\winvm\\sw-check-setup.exe -Version $version" \
    > "$log" 2>&1 9>&- || status=$?
  grep -E '^=== ' "$log" || true
  if [[ "$status" -ne 0 ]] || ! grep -q '^=== INSTALL CHECK GREEN' "$log"; then
    printf '%s\n' "vm-install-check: failed (ssh exit $status); read $log" >&2
    exit 1
  fi
}

main "$@"
