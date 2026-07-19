#!/usr/bin/env bash
# One-command release for the yunluoxin/gtab fork:
#   ./scripts/release.sh            # bump patch (1.8.0 -> 1.8.1) and release
#   ./scripts/release.sh 1.9.0      # release an explicit version
#
# Steps: bump Cargo.toml version -> commit -> tag -> push code+tag ->
# wait for the GitHub Actions release build -> regenerate the bottle formula
# (downloads the built tarballs' sha256) -> commit+push the tap repo.
# Other machines then just run: brew upgrade gtab
#
# Idempotent: if a previous run failed partway (e.g. after the tag was pushed
# but before the formula was updated), re-run with the same version and it
# resumes after the last completed step.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CODE_REMOTE="${GTAB_CODE_REMOTE:-origin}"
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

if git rev-parse "v${version}" >/dev/null 2>&1; then
  # Tag already exists locally: a previous run pushed the bump commit, so
  # resume at the workflow/formula steps instead of failing.
  if [[ "$current" != "$version" ]]; then
    echo "tag v${version} exists but Cargo.toml is at ${current}; resolve manually" >&2
    exit 1
  fi
  echo "v${version} already bumped and tagged; resuming release steps"
else
  if [[ "$version" == "$current" ]]; then
    echo "version $version equals current Cargo.toml version; nothing to do" >&2
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

  # 3. commit + tag
  git add Cargo.toml Cargo.lock
  git commit -m "chore: bump version to ${version}"
  git tag "v${version}"
fi

# push code + tag (pushing the tag triggers the release workflow); no-op if
# a previous run already pushed them
git push "$CODE_REMOTE" main "v${version}"

# 4. wait for the GitHub Actions release build to publish the tarballs
if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI is required to watch the release workflow" >&2
  exit 1
fi

echo "Waiting for the release workflow on yunluoxin/gtab..."
run_id=""
for _ in $(seq 1 24); do
  run_id="$(gh run list --repo yunluoxin/gtab --workflow release.yml \
    --branch "v${version}" --limit 1 --json databaseId --jq '.[0].databaseId' 2>/dev/null || true)"
  [[ -n "$run_id" ]] && break
  sleep 5
done

if [[ -z "$run_id" ]]; then
  echo "could not find the release workflow run for v${version}" >&2
  echo "check https://github.com/yunluoxin/gtab/actions and re-run the formula step manually:" >&2
  echo "  ./scripts/render-homebrew-formula.sh \"$TAP_DIR/Formula/gtab.rb\"" >&2
  exit 1
fi

if ! gh run watch "$run_id" --repo yunluoxin/gtab --exit-status --interval 15 >/dev/null; then
  echo "release workflow failed for v${version}" >&2
  echo "see: https://github.com/yunluoxin/gtab/actions/runs/${run_id}" >&2
  exit 1
fi

# 5. regenerate the bottle formula into the tap repo and push
./scripts/render-homebrew-formula.sh "$TAP_DIR/Formula/gtab.rb"
git -C "$TAP_DIR" add Formula/gtab.rb
if git -C "$TAP_DIR" diff --cached --quiet; then
  echo "formula unchanged; tap not updated"
else
  git -C "$TAP_DIR" commit -m "release v${version}"
  git -C "$TAP_DIR" push
fi

echo
echo "Released v${version} (bottles published by GitHub Actions)."
echo "Other machines: brew upgrade gtab"
