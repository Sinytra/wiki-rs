#!/usr/bin/env bash
set -euo pipefail

# Generate new types
cargo test -p wiki-domain --features ts

# Build index.ts
cd src
{
  for f in *.ts; do
    [ "$f" = "index.ts" ] && continue
    name="${f%.ts}"
    echo "export type { $name } from \"./$name\";"
  done
  for d in */; do
    dir="${d%/}"
    for f in "$dir"/*.ts; do
      [ -e "$f" ] || continue
      name="$(basename "$f" .ts)"
      echo "export type { $name } from \"./$dir/$name\";"
    done
  done
} > index.ts

# Format code
npm run format
