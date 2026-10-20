#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

wasm-pack build --release --target nodejs --scope sinytra --out-dir pkg

cd pkg
npm pkg set 'peerDependencies.@sinytra/wiki-api-types=^0.1.26'
