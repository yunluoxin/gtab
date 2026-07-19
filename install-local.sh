#!/bin/sh
# Build gtab from this source tree and install it over the Homebrew binary
# via a symlink, so iterating locally is just: ./install-local.sh
set -e
cd "$(dirname "$0")"

cargo build --release

GTAB_BIN="$(brew --prefix)/bin/gtab"
ln -sf "$PWD/target/release/gtab" "$GTAB_BIN"

echo "gtab installed -> $GTAB_BIN"
"$GTAB_BIN" --version
