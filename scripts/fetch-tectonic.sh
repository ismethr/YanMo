#!/bin/sh
# 下载 Tectonic 官方预编译程序到 ~/.yanmo/bin（YanMo 会自动在这里查找）。
# 用法：sh scripts/fetch-tectonic.sh [版本号]
set -e
VERSION="${1:-0.17.0}"
case "$(uname -s)-$(uname -m)" in
  Darwin-x86_64) TRIPLE=x86_64-apple-darwin ;;
  Darwin-arm64) TRIPLE=aarch64-apple-darwin ;;
  Linux-x86_64) TRIPLE=x86_64-unknown-linux-musl ;;
  Linux-aarch64) TRIPLE=aarch64-unknown-linux-musl ;;
  *) echo "不支持的平台：$(uname -s)-$(uname -m)，请从 https://github.com/tectonic-typesetting/tectonic/releases 手动下载" >&2; exit 1 ;;
esac
DEST="$HOME/.yanmo/bin"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
URL="https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40$VERSION/tectonic-$VERSION-$TRIPLE.tar.gz"
echo "下载 $URL"
curl -fsSL "$URL" -o "$TMP/tectonic.tar.gz"
tar -xzf "$TMP/tectonic.tar.gz" -C "$TMP"
mkdir -p "$DEST"
mv "$TMP/tectonic" "$DEST/tectonic"
chmod +x "$DEST/tectonic"
"$DEST/tectonic" --version
