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
} > index.ts

# Format code
npm run format
