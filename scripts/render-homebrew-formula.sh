#!/usr/bin/env bash
# Regenerate the homebrew formula for the current Cargo.toml version.
#
# Bottle mode (default): downloads the GitHub Actions release tarballs for
# the tag and embeds their sha256, so client machines install prebuilt
# binaries without a Rust toolchain. Requires the release workflow to have
# published assets for the tag first.
#
# Source mode: RENDER_SOURCE=1 ./scripts/render-homebrew-formula.sh
# falls back to building from the git tag on the client machine.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v git >/dev/null 2>&1; then
  echo "git is required" >&2
  exit 1
fi

REPO="yunluoxin/gtab"

version="$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -n 1)"
if [[ -z "$version" ]]; then
  echo "failed to read package version from Cargo.toml" >&2
  exit 1
fi

output="${1:-Formula/gtab.rb}"
mkdir -p "$(dirname "$output")"

caveats_and_test() {
  cat <<'EOF'

  def caveats
    <<~EOS
      Run this once to enable the default Ghostty-local Cmd+G:
        gtab init

      Workspaces are stored in ~/.config/gtab/ by default.
      Override with: export GTAB_DIR="/your/path"

      Requires Ghostty terminal: https://ghostty.org
    EOS
  end

  test do
    ENV["GTAB_DIR"] = testpath/"gtab"
    (testpath/"gtab").mkpath
    (testpath/"gtab/demo.applescript").write <<~APPLESCRIPT
      tell application "Ghostty"
      end tell
    APPLESCRIPT

    assert_match version.to_s, shell_output("#{bin}/gtab --version")
    assert_match "demo", shell_output("#{bin}/gtab list")
    assert_match "close_tab = off", shell_output("#{bin}/gtab set")
    assert_match "ghostty_shortcut = cmd+g", shell_output("#{bin}/gtab set")
    assert_match "Ghostty-local shortcut is the default fast path", shell_output("#{bin}/gtab set")

    system bin/"gtab", "set", "close_tab", "on"
    assert_match "close_tab = on", shell_output("#{bin}/gtab set")

    assert_match "launch_mode has been removed", shell_output("#{bin}/gtab set launch_mode window", 1)
  end
end
EOF
}

if [[ "${RENDER_SOURCE:-0}" == "1" ]]; then
  cat > "$output" <<EOF
class Gtab < Formula
  desc "Ghostty tab workspace manager with an interactive TUI"
  homepage "https://github.com/${REPO}"
  url "https://github.com/${REPO}.git",
      tag: "v${version}"
  version "${version}"
  license "MIT"
  head "https://github.com/${REPO}.git", branch: "main"

  depends_on :macos
  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end
$(caveats_and_test)
EOF
  echo "Wrote ${output} for v${version} (source build)"
  exit 0
fi

# Bottle mode: fetch sha256 for each released tarball. GitHub attaches
# release assets asynchronously after the workflow reports success, so retry
# on 404/empty responses instead of racing the upload.
fetch_sha() {
  local target="$1"
  local url="https://github.com/${REPO}/releases/download/v${version}/gtab-${version}-${target}.tar.gz.sha256"
  local sha="" attempt
  for attempt in $(seq 1 12); do
    if sha="$(curl -fsSL "$url" 2>/dev/null | awk '{print $1}')" && [[ -n "$sha" ]]; then
      break
    fi
    sha=""
    [[ "$attempt" -lt 12 ]] && sleep 10
  done
  if [[ -z "$sha" ]]; then
    echo "failed to fetch ${url} after 12 attempts" >&2
    echo "has the release workflow published v${version} assets yet?" >&2
    exit 1
  fi
  if [[ ! "$sha" =~ ^[0-9a-f]{64}$ ]]; then
    echo "unexpected sha256 content from ${url}: ${sha}" >&2
    exit 1
  fi
  printf '%s' "$sha"
}

arm_sha="$(fetch_sha aarch64-apple-darwin)"
intel_sha="$(fetch_sha x86_64-apple-darwin)"

cat > "$output" <<EOF
class Gtab < Formula
  desc "Ghostty tab workspace manager with an interactive TUI"
  homepage "https://github.com/${REPO}"
  version "${version}"
  license "MIT"
  head "https://github.com/${REPO}.git", branch: "main"

  on_arm do
    url "https://github.com/${REPO}/releases/download/v${version}/gtab-${version}-aarch64-apple-darwin.tar.gz"
    sha256 "${arm_sha}"
  end

  on_intel do
    url "https://github.com/${REPO}/releases/download/v${version}/gtab-${version}-x86_64-apple-darwin.tar.gz"
    sha256 "${intel_sha}"
  end

  depends_on :macos

  def install
    bin.install "gtab"
  end
$(caveats_and_test)
EOF

echo "Wrote ${output} for v${version} (bottle: arm=${arm_sha:0:12}... intel=${intel_sha:0:12}...)"
