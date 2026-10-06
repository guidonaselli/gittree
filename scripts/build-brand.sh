#!/usr/bin/env bash
# Regenera branding/ (SVG + PNG), los iconos de Tauri y los assets del frontend.
# Requiere rsvg-convert, magick, cargo-tauri y `pip install -r scripts/requirements-brand.txt`.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
b="$root/branding"
python3 "$root/scripts/build-brand.py"
mkdir -p "$b/png"

for s in 16 32 48 64 128 256 512 1024; do
  rsvg-convert -a -w "$s" -h "$s" "$b/mark.svg" -o "$b/png/mark-$s.png"
  magick "$b/png/mark-$s.png" -background none -gravity center -extent "${s}x${s}" "$b/png/mark-$s.png"
done
for v in "" -dark -mono -white; do
  rsvg-convert -w 1024 "$b/logo$v.svg" -o "$b/png/logo$v.png"
  rsvg-convert -w 1600 "$b/logo-horizontal$v.svg" -o "$b/png/logo-horizontal$v.png"
done
rsvg-convert -w 1024 -h 1024 "$b/app-icon.svg" -o "$b/png/app-icon-1024.png"
rsvg-convert -w 1024 "$b/wordmark.svg" -o "$b/png/wordmark.png"

rsvg-convert -w 960 "$b/logo-horizontal-dark.svg" -o "$b/png/_h.png"
magick -size 1280x640 xc:'#0B1230' "$b/png/_h.png" -gravity center -composite "$b/png/social-preview.png"
rm "$b/png/_h.png"

for s in 16 24 32; do
  rsvg-convert -w "$s" -h "$s" "$b/app-icon-tiny.svg" -o "$b/png/app-icon-$s.png"
done
rsvg-convert -w 48 -h 48 "$b/app-icon-small.svg" -o "$b/png/app-icon-48.png"
for v in "" -dark; do
  rsvg-convert -a -w 64 -h 64 "$b/mark-tiny$v.svg" -o "$b/png/mark-tiny$v-64.png"
  magick "$b/png/mark-tiny$v-64.png" -background none -gravity center -extent 64x64 "$b/png/mark-tiny$v-64.png"
done

(cd "$root/src-tauri" && cargo tauri icon "$b/png/app-icon-1024.png" >/dev/null)
rm -f "$root/src-tauri/icons/gittree.png" "$root/src-tauri/icons/logo.png"
cp "$b/png/app-icon-32.png" "$root/src-tauri/icons/32x32.png"
magick "$b/png/app-icon-16.png" "$b/png/app-icon-24.png" "$b/png/app-icon-32.png" "$b/png/app-icon-48.png" \
  \( "$b/png/app-icon-1024.png" -resize 64x64 \) \( "$b/png/app-icon-1024.png" -resize 128x128 \) \
  \( "$b/png/app-icon-1024.png" -resize 256x256 \) "$root/src-tauri/icons/icon.ico"

cp "$b/favicon.svg" "$root/frontend/public/favicon.svg"
cp "$b/png/mark-tiny-64.png" "$root/frontend/public/logo.png"
cp "$b/png/mark-tiny-dark-64.png" "$root/frontend/public/logo-dark.png"
