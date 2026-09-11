#!/usr/bin/env bash

set -euo pipefail

print_usage() {
    cat <<'USAGE'
Usage: scripts/check_fp.sh <path-to-file.fp>

Verifies a FerroPhase program by comparing interpreter and exec outputs and
generating a Rust projection.
USAGE
}

if [[ $# -lt 1 ]]; then
    print_usage >&2
    exit 1
fi

FP_FILE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h)
            print_usage
            exit 0
            ;;
        *)
            if [[ -z "$FP_FILE" ]]; then
                FP_FILE="$1"
            else
                echo "Unexpected argument: $1" >&2
                exit 1
            fi
            ;;
    esac
    shift
done

if [[ ! -f "$FP_FILE" ]]; then
    echo "FerroPhase file not found: $FP_FILE" >&2
    exit 1
fi

if ! command -v fp >/dev/null 2>&1; then
    echo "The 'fp' CLI is required but not found in PATH." >&2
    exit 1
fi

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

RUN_OUTPUT="$WORK_DIR/run.out"
EXEC_OUTPUT="$WORK_DIR/exec.out"

echo "→ Running interpreter (fp interpret)"
if ! fp interpret "$FP_FILE" >"$RUN_OUTPUT" 2>"$WORK_DIR/run.err"; then
    echo "fp interpret failed; see $WORK_DIR/run.err" >&2
    exit 1
fi

echo "→ Running exec path (fp exec --quiet --run --save-intermediates)"
if ! fp exec --quiet --run --save-intermediates "$FP_FILE" >"$EXEC_OUTPUT" 2>"$WORK_DIR/exec.err"; then
    echo "fp exec failed; see $WORK_DIR/exec.err" >&2
    exit 1
fi

if ! diff -u "$RUN_OUTPUT" "$EXEC_OUTPUT" >"$WORK_DIR/output.diff"; then
    echo "Interpreter and exec outputs differ:" >&2
    cat "$WORK_DIR/output.diff" >&2
    exit 1
fi
echo "✓ Interpreter and exec outputs match"

echo "→ Transpiling / compiling to Rust"

RUST_OUT="$WORK_DIR/$(basename "$FP_FILE" .fp).rs"

if ! fp compile "$FP_FILE" --target rust --output "$RUST_OUT" >"$WORK_DIR/rust.log" 2>&1; then
    echo "fp compile --target rust failed; see $WORK_DIR/rust.log" >&2
    exit 1
fi

echo "✓ Generated Rust:    $RUST_OUT"
echo "✅ All checks completed successfully"
