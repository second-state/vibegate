#!/usr/bin/env bash
# Build the single-file release: bundle the web frontend, then bake it into
# the binary (rust-embed embeds web/dist at compile time). The resulting
# binary is self-contained — copy it anywhere and run.
#
# Usage: build-release.sh [cargo-target-triple]
#   ./build-release.sh                              # host target
#   ./build-release.sh x86_64-unknown-linux-musl    # cross-compile (CI)
set -euo pipefail
cd "$(dirname "$0")"

TARGET="${1:-}"

echo "==> building web frontend"
if [ ! -d web/node_modules ]; then
  echo "    node_modules missing — installing dependencies"
  (cd web && npm install)
fi
(cd web && npm run build)

echo "==> building release binary${TARGET:+ for $TARGET}"
if [ -n "$TARGET" ]; then
  cargo build --release --target "$TARGET"
  BIN="target/$TARGET/release/vibegate"
else
  cargo build --release
  BIN="target/release/vibegate"
fi
# Windows targets produce vibegate.exe
if [ -f "$BIN.exe" ]; then
  BIN="$BIN.exe"
fi

echo
ls -lh "$BIN"

# Expose the artifact path to GitHub Actions steps (harmless locally).
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "bin=$BIN" >> "$GITHUB_OUTPUT"
fi
echo "done: $BIN (run it from anywhere, no web dir needed)"
