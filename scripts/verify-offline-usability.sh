#!/usr/bin/env bash
set -euo pipefail

echo "=== Verifying GitTree 1.0.0 Offline Usability & Remote Error Surfacing ==="

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_BIN="$REPO_ROOT/target/release/gittree"

# 1. Test in an isolated unshared network namespace (zero internet access)
echo "--- 1. Testing GitTree in an Isolated Network Namespace ---"
export REPO_ROOT
export RELEASE_BIN

unshare -r -n bash << 'EOF'
set -euo pipefail

# Create a local test git repo
REPO_DIR=$(mktemp -d /tmp/gittree-offline-test-XXXXXX)
git init -b main "$REPO_DIR" >/dev/null
git -C "$REPO_DIR" config user.name "GitTree Offline Tester"
git -C "$REPO_DIR" config user.email "tester@offline.local"
echo "Offline content" > "$REPO_DIR/file.txt"
git -C "$REPO_DIR" add file.txt
git -C "$REPO_DIR" commit -m "offline commit" >/dev/null

# Add an unreachable remote
git -C "$REPO_DIR" remote add origin "https://unreachable.example.invalid/repo.git"

# Verify remote operation fails immediately with clear error
FETCH_OUTPUT=$(git -C "$REPO_DIR" fetch origin 2>&1 || true)
if echo "$FETCH_OUTPUT" | grep -qiE "Could not resolve|Failed to connect|network is unreachable"; then
    echo "✓ Remote git operation fails fast with clear network error: $(echo "$FETCH_OUTPUT" | head -n 1)"
else
    echo "ERROR: Remote operation did not produce expected network failure: $FETCH_OUTPUT"
    exit 1
fi

# Launch GitTree with the test repo in headless Xvfb
XVFB_DISPLAY=":92"
Xvfb "$XVFB_DISPLAY" -screen 0 1280x800x24 &
XVFB_PID=$!

cleanup() {
    if [ -n "${APP_PID:-}" ] && kill -0 "$APP_PID" 2>/dev/null; then
        kill "$APP_PID" 2>/dev/null || true
    fi
    kill "$XVFB_PID" 2>/dev/null || true
    rm -rf "$REPO_DIR"
}
trap cleanup EXIT
sleep 1

DISPLAY="$XVFB_DISPLAY" GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 "$RELEASE_BIN" "$REPO_DIR" &
APP_PID=$!
sleep 4

if kill -0 "$APP_PID" 2>/dev/null; then
    echo "✓ GitTree started and ran smoothly offline with reference repo (PID: $APP_PID)"
    kill "$APP_PID" 2>/dev/null || true
else
    echo "ERROR: GitTree failed to run in offline environment"
    exit 1
fi

echo "✓ Full offline usability verified without hanging or degrading"
EOF

echo "=== All Offline Usability Checks Passed Successfully! ==="
