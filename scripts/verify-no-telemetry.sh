#!/usr/bin/env bash
set -euo pipefail

echo "=== Verifying GitTree 1.0.0 No-Telemetry Guarantee ==="

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_BIN="$REPO_ROOT/target/release/gittree"

# 1. Static Audit: Rust dependencies
echo "--- 1. Static Audit: Dependencies & Configuration ---"

SUSPECT_CRATES=("reqwest" "ureq" "hyper" "sentry" "posthog" "datadog" "telemetry" "segment")
for crate in "${SUSPECT_CRATES[@]}"; do
    if cargo tree --edges no-build -i "$crate" 2>/dev/null | grep -q "$crate"; then
        echo "FAIL: Prohibited runtime network/telemetry crate found: $crate"
        exit 1
    fi
done
echo "✓ Zero prohibited runtime networking or telemetry crates in dependency graph"

# 2. Static Audit: Frontend dependencies
SUSPECT_JS=("@sentry" "analytics" "mixpanel" "posthog" "datadog" "segment" "gtag")
for pkg in "${SUSPECT_JS[@]}"; do
    if grep -qi "$pkg" "$REPO_ROOT/frontend/package.json"; then
        echo "FAIL: Prohibited telemetry package found in package.json: $pkg"
        exit 1
    fi
done
echo "✓ Zero telemetry or analytics packages in frontend/package.json"

# 3. Static Audit: Tauri CSP & Updater
if ! grep -q "default-src 'self'" "$REPO_ROOT/src-tauri/tauri.conf.json"; then
    echo "FAIL: CSP in tauri.conf.json does not restrict default-src to 'self'"
    exit 1
fi

if grep -qi "updater" "$REPO_ROOT/src-tauri/tauri.conf.json"; then
    echo "FAIL: Auto-updater configuration detected in tauri.conf.json"
    exit 1
fi
echo "✓ Strict 'self' CSP enforced and auto-updater disabled in tauri.conf.json"

# 4. Dynamic Audit: Runtime outbound socket check
echo "--- 2. Dynamic Audit: Runtime Network Sockets ---"
XVFB_DISPLAY=":94"
Xvfb "$XVFB_DISPLAY" -screen 0 1280x800x24 &
XVFB_PID=$!

cleanup() {
    if [ -n "${APP_PID:-}" ] && kill -0 "$APP_PID" 2>/dev/null; then
        kill "$APP_PID" 2>/dev/null || true
    fi
    kill "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT

DISPLAY="$XVFB_DISPLAY" GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 "$RELEASE_BIN" &
APP_PID=$!
sleep 5

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
PID_LIST=$(echo "$TREE_PIDS" | tr '\n' ',' | sed 's/,$//')

if [ -n "$PID_LIST" ]; then
    OPEN_SOCKETS=$(lsof -i -a -p "$PID_LIST" 2>/dev/null || true)
    if [ -n "$OPEN_SOCKETS" ]; then
        echo "FAIL: Detected open network socket(s) in GitTree process tree:"
        echo "$OPEN_SOCKETS"
        exit 1
    fi
fi

echo "✓ Verified zero outbound or listening network sockets at runtime"
echo "=== No-Telemetry Guarantee Verified Successfully! ==="
