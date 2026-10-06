#!/usr/bin/env bash
set -euo pipefail

echo "=== Verifying GitTree 1.0.0 Release Budgets ==="

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_BIN="$REPO_ROOT/target/release/gittree"

# 1. Binary Size Budget (≤ 40 MB = 41,943,040 bytes)
BINARY_BUDGET_BYTES=41943040
if [ ! -f "$RELEASE_BIN" ]; then
    echo "ERROR: Release binary not found at $RELEASE_BIN"
    exit 1
fi

ACTUAL_BYTES=$(stat -c%s "$RELEASE_BIN")
ACTUAL_MB=$(awk "BEGIN {printf \"%.2f\", $ACTUAL_BYTES / 1048576}")
echo "--- 1. Binary Size Audit ---"
echo "Binary size budget: 40.00 MB ($BINARY_BUDGET_BYTES bytes)"
echo "Actual binary size: $ACTUAL_MB MB ($ACTUAL_BYTES bytes)"

if [ "$ACTUAL_BYTES" -gt "$BINARY_BUDGET_BYTES" ]; then
    echo "FAIL: Release binary exceeds 40 MB budget!"
    exit 1
fi
echo "✓ Binary size gate PASSED ($ACTUAL_MB MB ≤ 40.00 MB)"

# 2. Idle Memory Budget (≤ 250 MB with 25-submodule reference superproject open)
echo "--- 2. Idle Memory Audit (Reference Superproject) ---"
MEMORY_BUDGET_MB=250

FIXTURE_DIR=$(mktemp -d /tmp/gittree-budget-fixture-XXXXXX)
echo "Generating 25-submodule reference superproject in $FIXTURE_DIR..."
cargo run --release -p repo-state --bin fixture_gen superproject "$FIXTURE_DIR" 25 >/dev/null

XVFB_DISPLAY=":98"
echo "Launching Xvfb on $XVFB_DISPLAY..."
Xvfb "$XVFB_DISPLAY" -screen 0 1280x800x24 &
XVFB_PID=$!

cleanup() {
    echo "Cleaning up..."
    if [ -n "${APP_PID:-}" ] && kill -0 "$APP_PID" 2>/dev/null; then
        kill "$APP_PID" 2>/dev/null || true
    fi
    kill "$XVFB_PID" 2>/dev/null || true
    rm -rf "$FIXTURE_DIR"
}
trap cleanup EXIT

# Pre-populate localStorage with the superproject open in gittree.workspace
# We can also pass the path directly as argument to GitTree
echo "Launching GitTree with reference superproject..."
DISPLAY="$XVFB_DISPLAY" GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 "$RELEASE_BIN" "$FIXTURE_DIR" &
APP_PID=$!

# Allow 7 seconds for the app to initialize, open the repo, compute status & matrix, and settle into idle
sleep 7

if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "ERROR: GitTree process exited prematurely"
    exit 1
fi

get_tree_pids() {
    local parent=$1
    echo "$parent"
    local children
    children=$(pgrep -P "$parent" 2>/dev/null || true)
    for child in $children; do
        get_tree_pids "$child"
    done
}

TREE_PIDS=$(get_tree_pids "$APP_PID" | sort -u)
TOTAL_PSS_KB=0
TOTAL_RSS_KB=0

echo "Processes in GitTree tree:"
for pid in $TREE_PIDS; do
    CMD=""
    if [ -f "/proc/$pid/cmdline" ]; then
        CMD=$(cat "/proc/$pid/cmdline" | tr '\0' ' ' | head -c 60 || true)
    fi
    RSS_KB=0
    if [ -f "/proc/$pid/status" ]; then
        RSS_KB=$(awk '/VmRSS/{print $2}' "/proc/$pid/status" || echo 0)
    fi
    PSS_KB=$RSS_KB
    if [ -f "/proc/$pid/smaps_rollup" ]; then
        PSS_KB=$(awk '/^Pss:/{print $2}' "/proc/$pid/smaps_rollup" || echo "$RSS_KB")
    fi
    echo "  PID $pid ($CMD): PSS ${PSS_KB} KB (RSS ${RSS_KB} KB)"
    TOTAL_PSS_KB=$((TOTAL_PSS_KB + PSS_KB))
    TOTAL_RSS_KB=$((TOTAL_RSS_KB + RSS_KB))
done

TOTAL_PSS_MB=$(awk "BEGIN {printf \"%.2f\", $TOTAL_PSS_KB / 1024}")
TOTAL_RSS_MB=$(awk "BEGIN {printf \"%.2f\", $TOTAL_RSS_KB / 1024}")
echo "Total Idle PSS: $TOTAL_PSS_MB MB ($TOTAL_PSS_KB KB) [Raw RSS sum: $TOTAL_RSS_MB MB]"
echo "Memory budget:  $MEMORY_BUDGET_MB MB"

if (( $(awk "BEGIN {print ($TOTAL_PSS_MB > $MEMORY_BUDGET_MB)}") )); then
    echo "FAIL: Idle memory ($TOTAL_PSS_MB MB) exceeds budget ($MEMORY_BUDGET_MB MB)"
    exit 1
fi

echo "✓ Idle memory gate PASSED ($TOTAL_PSS_MB MB ≤ $MEMORY_BUDGET_MB MB)"

# Cleanly terminate GitTree
kill "$APP_PID" 2>/dev/null || true
wait "$APP_PID" 2>/dev/null || true

echo "=== All Release Budgets Verified Successfully! ==="
