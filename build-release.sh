#!/usr/bin/env bash
# Build the single-file release: bundle the web frontend, then bake it into
# the binary (rust-embed embeds web/dist at compile time). The resulting
# target/release/vibegate is self-contained — copy it anywhere and run.
set -euo pipefail
cd "$(dirname "$0")"

echo "==> building web frontend"
if [ ! -d web/node_modules ]; then
  echo "    node_modules missing — installing dependencies"
  (cd web && npm install)
fi
(cd web && npm run build)

echo "==> building release binary"
cargo build --release

echo
ls -lh target/release/vibegate
echo "done: target/release/vibegate (run it from anywhere, no web dir needed)"
