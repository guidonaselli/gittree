#!/usr/bin/env bash
set -euo pipefail

echo "=== Verifying GitTree 1.0.0 Packaging Artifacts ==="

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPIMAGE="$REPO_ROOT/target/release/bundle/appimage/GitTree_1.0.0_amd64.AppImage"
ARCH_PKG="$REPO_ROOT/packaging/arch/gittree-1.0.0-1-x86_64.pkg.tar.zst"

# 1. Assert artifact existence
if [ ! -f "$APPIMAGE" ]; then
    echo "ERROR: AppImage not found at $APPIMAGE"
    exit 1
fi

if [ ! -f "$ARCH_PKG" ]; then
    echo "ERROR: Arch package not found at $ARCH_PKG"
    exit 1
fi

echo "✓ Found AppImage: $(ls -lh "$APPIMAGE" | awk '{print $5, $9}')"
echo "✓ Found Arch Package: $(ls -lh "$ARCH_PKG" | awk '{print $5, $9}')"

# 2. Verify Arch package structure
echo "--- Verifying Arch Package Contents ---"
ARCH_CONTENTS=$(tar -tf "$ARCH_PKG")
echo "$ARCH_CONTENTS" | grep -q "usr/bin/gittree" || { echo "ERROR: Arch package missing usr/bin/gittree"; exit 1; }
echo "$ARCH_CONTENTS" | grep -q "usr/share/applications/gittree.desktop" || { echo "ERROR: Arch package missing desktop file"; exit 1; }
echo "$ARCH_CONTENTS" | grep -q "usr/share/icons/hicolor/512x512/apps/gittree.png" || { echo "ERROR: Arch package missing 512x512 icon"; exit 1; }
echo "$ARCH_CONTENTS" | grep -q "usr/share/licenses/gittree/LICENSE" || { echo "ERROR: Arch package missing LICENSE"; exit 1; }
echo "✓ Arch package contains binary, desktop entry, hicolor icons, and license"

# 3. Test execution under headless X11 (Xvfb)
XVFB_DISPLAY=":99"
echo "--- Launching Xvfb on $XVFB_DISPLAY ---"
Xvfb "$XVFB_DISPLAY" -screen 0 1280x800x24 &
XVFB_PID=$!
trap 'kill $XVFB_PID 2>/dev/null || true' EXIT
sleep 1

# Smoke test 1: AppImage under X11
echo "--- Testing AppImage launch under X11 ---"
DISPLAY="$XVFB_DISPLAY" GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 "$APPIMAGE" &
APP_PID=$!
sleep 3
if kill -0 "$APP_PID" 2>/dev/null; then
    echo "✓ AppImage launched and ran successfully under X11 (PID: $APP_PID)"
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
else
    echo "ERROR: AppImage exited unexpectedly"
    exit 1
fi

# Smoke test 2: Arch package binary under X11
echo "--- Testing Arch package binary launch under X11 ---"
EXTRACT_DIR=$(mktemp -d)
trap 'kill $XVFB_PID 2>/dev/null || true; rm -rf "$EXTRACT_DIR"' EXIT
tar -xf "$ARCH_PKG" -C "$EXTRACT_DIR"

DISPLAY="$XVFB_DISPLAY" GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 "$EXTRACT_DIR/usr/bin/gittree" &
ARCH_APP_PID=$!
sleep 3
if kill -0 "$ARCH_APP_PID" 2>/dev/null; then
    echo "✓ Arch package binary launched and ran successfully under X11 (PID: $ARCH_APP_PID)"
    kill "$ARCH_APP_PID" 2>/dev/null || true
    wait "$ARCH_APP_PID" 2>/dev/null || true
else
    echo "ERROR: Arch package binary exited unexpectedly"
    exit 1
fi

echo "=== All packaging smoke tests passed successfully! ==="
