#!/bin/sh
# 把 assets/brand/icon.svg 渲染为透明背景的 1024px PNG，并生成各平台图标与官网用图。
# 需要 Google Chrome（无头模式渲染，保留透明度）。注意不要用 QuickLook 渲染：它会填上白底，图标会出现白边。
set -e
cd "$(dirname "$0")/.."
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cat > "$TMP/icon.html" <<HTML
<!doctype html><html><head><style>html,body{margin:0;background:transparent}img{display:block;width:1024px;height:1024px}</style></head>
<body><img src="file://$PWD/assets/brand/icon.svg"></body></html>
HTML
"$CHROME" --headless=new --disable-gpu --hide-scrollbars --default-background-color=00000000 \
  --force-device-scale-factor=1 --window-size=1024,1024 --allow-file-access-from-files \
  --screenshot="$PWD/assets/brand/icon-1024.png" "file://$TMP/icon.html" 2>/dev/null
pnpm --dir apps/desktop/ui -s tauri icon ../../../assets/brand/icon-1024.png -o ../src-tauri/icons >/dev/null 2>&1
rm -rf apps/desktop/src-tauri/icons/android apps/desktop/src-tauri/icons/ios
cp assets/brand/icon.svg apps/desktop/ui/public/icon.svg
cp assets/brand/icon.svg site/assets/icon.svg
sips -Z 512 assets/brand/icon-1024.png --out site/assets/icon-512.png >/dev/null
sips -Z 180 assets/brand/icon-1024.png --out site/assets/apple-touch-icon.png >/dev/null
echo "图标已更新"
