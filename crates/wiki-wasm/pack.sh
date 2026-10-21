#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

wasm-pack build --release --target nodejs --scope sinytra --out-dir pkg

version="${WASM_VERSION:-}"
if [[ -z "$version" ]]; then
  described="$(git describe --tags --long --match 'v[0-9]*.[0-9]*' 2>/dev/null || true)"
  if [[ "$described" =~ ^v([0-9]+\.[0-9]+)(\.[0-9]+)?-([0-9]+)-g ]]; then
    version="${BASH_REMATCH[1]}.${BASH_REMATCH[3]}"
  fi
fi

cd pkg
npm pkg set 'peerDependencies.@sinytra/wiki-api-types=^0.1.26'
if [[ -n "$version" ]]; then
  npm pkg set "version=$version"
fi
if [[ -n "${GITHUB_REPOSITORY:-}" ]]; then
  npm pkg set "repository.url=git+${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}.git" 'repository.directory=crates/wiki-wasm'
fi

echo "Packed $(node -p 'const p = require("./package.json"); `${p.name}@${p.version}`')"
