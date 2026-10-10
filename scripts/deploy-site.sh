#!/bin/sh
# 把 site/ 部署到自托管服务器上 nginx 的站点目录（再由 Cloudflare Tunnel 发布为 https://yanmo.ink）。
# 用法：YANMO_SITE_HOST=user@host sh scripts/deploy-site.sh
# 服务器一次性配置见 docs/site-hosting.md。
set -e
: "${YANMO_SITE_HOST:?请设置 YANMO_SITE_HOST，例如 user@192.168.1.2}"
DIR=/var/www/yanmo.ink
cd "$(dirname "$0")/.."
# 先解压到临时目录再整体替换，避免访问者看到更新到一半的页面
tar -C site --exclude='.DS_Store' -czf - . | ssh "$YANMO_SITE_HOST" "set -e
  sudo rm -rf $DIR.new && sudo mkdir -p $DIR.new
  sudo tar -xzf - -C $DIR.new
  sudo chown -R root:root $DIR.new && sudo chmod -R a+rX,go-w $DIR.new
  sudo rm -rf $DIR.old
  if [ -d $DIR ]; then sudo mv $DIR $DIR.old; fi
  sudo mv $DIR.new $DIR"
echo "已部署到 $YANMO_SITE_HOST:$DIR"
