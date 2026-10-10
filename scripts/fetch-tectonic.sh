#!/bin/sh
# 下载 Tectonic 官方预编译程序。
#
#   sh scripts/fetch-tectonic.sh                    # 开发用：装到 ~/.yanmo/bin（YanMo 会在这里查找）
#   sh scripts/fetch-tectonic.sh --sidecar [triple] # 打包用：放到 apps/desktop/src-tauri/binaries/tectonic-<triple>
#
# 环境变量 TECTONIC_VERSION 可指定版本（默认 0.17.0）。
set -e
VERSION="${TECTONIC_VERSION:-0.17.0}"
MODE=dev
TRIPLE=""
if [ "$1" = "--sidecar" ]; then
  MODE=sidecar
  TRIPLE="$2"
fi

if [ -z "$TRIPLE" ]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-x86_64) TRIPLE=x86_64-apple-darwin ;;
    Darwin-arm64) TRIPLE=aarch64-apple-darwin ;;
    Linux-x86_64) TRIPLE=x86_64-unknown-linux-gnu ;;
    Linux-aarch64) TRIPLE=aarch64-unknown-linux-gnu ;;
    MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64) TRIPLE=x86_64-pc-windows-msvc ;;
    *) echo "不支持的平台：$(uname -s)-$(uname -m)；请从 https://github.com/tectonic-typesetting/tectonic/releases 手动下载" >&2; exit 1 ;;
  esac
fi

# Tectonic 发布的资源名：Linux 用静态链接的 musl 版本，兼容性最好
case "$TRIPLE" in
  x86_64-unknown-linux-gnu) ASSET_TRIPLE=x86_64-unknown-linux-musl ;;
  aarch64-unknown-linux-gnu) ASSET_TRIPLE=aarch64-unknown-linux-musl ;;
  *) ASSET_TRIPLE="$TRIPLE" ;;
esac
case "$TRIPLE" in
  *windows*) EXT=zip; EXE=tectonic.exe ;;
  *) EXT=tar.gz; EXE=tectonic ;;
esac

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
URL="https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40$VERSION/tectonic-$VERSION-$ASSET_TRIPLE.$EXT"
echo "下载 $URL"
curl -fsSL "$URL" -o "$TMP/pkg.$EXT"
if [ "$EXT" = zip ]; then
  unzip -q "$TMP/pkg.$EXT" -d "$TMP"
else
  tar -xzf "$TMP/pkg.$EXT" -C "$TMP"
fi

if [ "$MODE" = sidecar ]; then
  DEST_DIR="$(cd "$(dirname "$0")/.." && pwd)/apps/desktop/src-tauri/binaries"
  case "$TRIPLE" in
    *windows*) DEST="$DEST_DIR/tectonic-$TRIPLE.exe" ;;
    *) DEST="$DEST_DIR/tectonic-$TRIPLE" ;;
  esac
else
  DEST_DIR="$HOME/.yanmo/bin"
  DEST="$DEST_DIR/$EXE"
fi
mkdir -p "$DEST_DIR"
mv "$TMP/$EXE" "$DEST"
chmod +x "$DEST"
echo "已安装到 $DEST"
