#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

cargo build -p fp-cli

export FP_BIN="${ROOT}/target/debug/fp"
OUT_DIR="${ROOT}/target/bootstrap/build"
mkdir -p "${OUT_DIR}"

# NOTE: bootstrap mode compiles a single entrypoint without a workspace graph
# and verifies the produced native executable exists.
"${FP_BIN}" compile "${ROOT}/src/bin/fptest.fp" \
  --backend binary \
  --output "${OUT_DIR}/fptest.out"

echo "Build artifacts: ${OUT_DIR}"
ls -la "${OUT_DIR}"
