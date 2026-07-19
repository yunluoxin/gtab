#!/usr/bin/env bash
# One-command release for the yunluoxin/gtab fork:
#   ./scripts/release.sh            # bump patch (1.8.0 -> 1.8.1) and release
#   ./scripts/release.sh 1.9.0      # release an explicit version
#
# Steps: bump Cargo.toml version -> commit -> tag -> push code+tag ->
# regenerate the homebrew formula -> commit+push the tap repo.
# Other machines then just run: brew upgrade gtab
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CODE_REMOTE="${GTAB_CODE_REMOTE:-myfork}"
TAP_DIR="${GTAB_TAP_DIR:-$(brew --repo yunluoxin/gtab 2>/dev/null || true)}"

if [[ ! -d "$TAP_DIR/.git" ]]; then
  echo "tap repo not found at '$TAP_DIR'" >&2
  echo "set GTAB_TAP_DIR to a local checkout of yunluoxin/homebrew-gtab" >&2
  exit 1
fi

current="$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -n 1)"
if [[ -z "$current" ]]; then
  echo "failed to read version from Cargo.toml" >&2
  exit 1
fi

if [[ $# -ge 1 ]]; then
  version="$1"
else
  # bump patch component
  major="${current%%.*}"
  rest="${current#*.}"
  minor="${rest%%.*}"
  patch="${rest#*.}"
  version="${major}.${minor}.$((patch + 1))"
fi

if [[ "$version" == "$current" ]]; then
  echo "version $version equals current Cargo.toml version; nothing to do" >&2
  exit 1
fi

if git rev-parse "v${version}" >/dev/null 2>&1; then
  echo "tag v${version} already exists" >&2
  exit 1
fi

if [[ -n "$(git status --porcelain -- Cargo.toml Cargo.lock)" ]]; then
  echo "Cargo.toml/Cargo.lock have uncommitted changes; commit or stash first" >&2
  exit 1
fi

echo "Releasing v${version} (current: v${current})"

# 1. bump version
sed -i '' "s/^version = \"${current}\"/version = \"${version}\"/" Cargo.toml
cargo update -p gtab --quiet

# 2. verify it still builds and tests pass
cargo test --quiet

# 3. commit + tag + push code
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to ${version}"
git tag "v${version}"
git push "$CODE_REMOTE" main "v${version}"

# 4. regenerate formula into the tap repo and push
./scripts/render-homebrew-formula.sh "$TAP_DIR/Formula/gtab.rb"
git -C "$TAP_DIR" add Formula/gtab.rb
if git -C "$TAP_DIR" diff --cached --quiet; then
  echo "formula unchanged; tap not updated"
else
  git -C "$TAP_DIR" commit -m "release v${version}"
  git -C "$TAP_DIR" push
fi

echo
echo "Released v${version}."
echo "Other machines: brew upgrade gtab"
