#!/usr/bin/env bash
# The guide's playable demos, built to wasm and dropped into the book.
#
#   scripts/build-demos.sh            build every demo
#   scripts/build-demos.sh step02_light   build one
#
# Each demo is a tutorial binary built for the browser with trunk, which
# fetches a wasm-bindgen matching the lockfile and runs wasm-opt over the
# result. Output goes to docs/guide/src/demos/<name>/, which mdBook copies
# into the book as static files, so a chapter embeds one in an iframe and
# nothing loads until a reader asks for it.
set -euo pipefail
cd "$(dirname "$0")/.."

# The generated index is scaffolding; a failed build should not leave it.
index="examples/tutorial/demo-index.html"
trap 'rm -f "$index"' EXIT

demos=(step01_walking step02_light step03_blows step04_things step05_knack step06_descent)
[[ $# -gt 0 ]] && demos=("$@")

for name in "${demos[@]}"; do
  echo "== $name"
  cat > "$index" <<HTML
<!DOCTYPE html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>$name</title>
    <link data-trunk rel="rust" data-bin="$name" data-wasm-opt="z" />
    <style>
      html, body { margin: 0; height: 100%; background: #000; overflow: hidden; }
      canvas { display: block; margin: 0 auto; }
    </style>
  </head>
  <body></body>
</html>
HTML
  ( cd examples/tutorial && trunk build demo-index.html --release --filehash false --public-url ./ --dist "../../docs/guide/src/demos/$name" )
  mv "docs/guide/src/demos/$name/demo-index.html" "docs/guide/src/demos/$name/index.html" 2>/dev/null || true
  du -h "docs/guide/src/demos/$name"/*.wasm
done
echo "demos built into docs/guide/src/demos/"
