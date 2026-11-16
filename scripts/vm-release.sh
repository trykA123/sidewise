#!/usr/bin/env bash
# Builds the release zip and SHA256SUMS for one commit in the Windows VM (atom REL3).
# It runs vm-build.sh's release stage, copies sidewise.exe back and writes <repo>/dist/.
set -euo pipefail

readonly LOCK_WAIT_S=3600
readonly REMOTE_EXE='C:/src/navo-compare/target/release/sidewise.exe'
readonly TEXT_FILES=(README.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE)

usage() {
  printf '%s\n' "usage: WINVM=<path to winvm.sh> vm-release.sh <rev>" >&2
}

# Prints a one-line error and exits 1.
die() {
  printf '%s\n' "vm-release: $*" >&2
  exit 1
}

# Prints the navo-compare-app package version at <sha>; it may be inherited from the workspace.
package_version() {
  local repo="$1" sha="$2" version file
  for file in crates/navo-compare-app/Cargo.toml Cargo.toml; do
    version="$(git -C "$repo" show "$sha:$file" \
      | sed -n 's/^version = "\(.*\)"$/\1/p' | head -n 1)"
    [[ -n "$version" ]] && { printf '%s\n' "$version"; return; }
  done
  die "no package version at $sha"
}

# Builds <sha> in the VM, copies the exe to <exe> and zips it with the text files into dist/.
# Runs under the VM lock, so no other job can replace the exe between build and copy.
build_and_pack() {
  local repo="$1" sha="$2" winvm="$3" version="$4" stage="$5"
  local name="sidewise-$version-windows-x64" script_dir="$repo/scripts" f
  local dist="$repo/dist"
  VM_LOCK_HELD=1 "$script_dir/vm-build.sh" "$sha" release || die "VM build failed"
  "$winvm" scp "admin@127.0.0.1:$REMOTE_EXE" "$stage/sidewise.exe" || die "copying the exe failed"
  [[ -s "$stage/sidewise.exe" ]] || die "sidewise.exe is missing or empty"
  for f in "${TEXT_FILES[@]}"; do
    git -C "$repo" show "$sha:$f" > "$stage/$f" || die "cannot read $f at $sha"
  done
  mkdir -p "$dist"
  rm -f -- "${dist:?}/$name.zip" "$dist/SHA256SUMS"
  (cd "$stage" && bsdtar -a -cf "$dist/$name.zip" sidewise.exe "${TEXT_FILES[@]}") \
    || die "zip failed"
  (cd "$dist" && sha256sum -- "$name.zip" > SHA256SUMS) || die "checksum failed"
}

# Builds <rev> in the VM and writes the zip and SHA256SUMS to dist/.
main() {
  if [[ "${1:-}" == "-h" ]]; then usage; exit 0; fi
  if [[ $# -ne 1 || "$1" == -* ]]; then usage; exit 2; fi
  local winvm="${WINVM:?set WINVM to the path of winvm.sh}"
  local logs="${WINVM_LOGS:-$HOME/.cache/winvm-ci}"
  local script_dir repo sha version
  command -v bsdtar >/dev/null || die "bsdtar missing"
  script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
  repo="$(git -C "$script_dir" rev-parse --show-toplevel)"
  sha="$(git -C "$repo" rev-parse --verify "$1^{commit}")" || die "unknown rev $1"
  version="$(package_version "$repo" "$sha")"
  mkdir -p "$logs"

  exec 9>"$logs/.vm.lock"
  flock -w "$LOCK_WAIT_S" 9 || { printf '%s\n' "vm-release: the VM stayed busy for ${LOCK_WAIT_S} s" >&2; exit 75; }

  tmp="$(mktemp -d)"
  trap 'rm -rf -- "$tmp"' EXIT
  build_and_pack "$repo" "$sha" "$winvm" "$version" "$tmp"
  printf '%s\n' "vm-release: wrote $repo/dist/sidewise-$version-windows-x64.zip"
}

main "$@"
