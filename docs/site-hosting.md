# 官网 yanmo.ink 的托管

官网是 `site/` 下的纯静态页面（不引用任何外部字体或脚本，国内可正常访问），由一台自托管的 Ubuntu 机器上的 nginx 提供，再通过 Cloudflare Tunnel 发布到公网。

## 服务器一次性配置

```bash
sudo apt-get install -y nginx
sudo rm -f /etc/nginx/sites-enabled/default        # 关闭默认的 80 端口站点
sudo mkdir -p /var/www/yanmo.ink
```

`/etc/nginx/sites-available/yanmo.ink`：

```nginx
server {
    listen 8080;
    listen [::]:8080;
    server_name yanmo.ink www.yanmo.ink _;

    root /var/www/yanmo.ink;
    index index.html;
    charset utf-8;
    server_tokens off;

    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    gzip on;
    gzip_types text/css application/javascript image/svg+xml;

    location / {
        try_files $uri $uri/ =404;
    }

    location /assets/ {
        expires 7d;
        add_header Cache-Control "public" always;
        add_header X-Content-Type-Options "nosniff" always;
    }
}
```

```bash
sudo ln -sf /etc/nginx/sites-available/yanmo.ink /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl restart nginx
```

## 发布更新

```bash
YANMO_SITE_HOST=user@host sh scripts/deploy-site.sh
```

脚本先解压到临时目录再整体替换，访问者不会看到更新到一半的页面；上一版保留在 `/var/www/yanmo.ink.old`。

## Cloudflare Tunnel

在 Cloudflare Zero Trust → Networks → Tunnels 中给隧道添加 Public hostname：

| 字段 | 值 |
|---|---|
| Hostname | `yanmo.ink`（可再加一条 `www.yanmo.ink`） |
| Service | `HTTP` · `localhost:8080` |

使用配置文件时，对应的 ingress 为：

```yaml
ingress:
  - hostname: yanmo.ink
    service: http://localhost:8080
  - hostname: www.yanmo.ink
    service: http://localhost:8080
  - service: http_status:404
```

建议在 Cloudflare 的 SSL/TLS 设置中开启 **Always Use HTTPS**。

## 更新截图

首页截图来自界面的演示模式（浏览器预览 + 示例论文）：

```bash
pnpm --dir apps/desktop/ui dev &
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --hide-scrollbars \
  --force-device-scale-factor=2 --window-size=1440,860 --virtual-time-budget=4000 \
  --screenshot=app.png "http://localhost:5173/?demo"
sips -s format jpeg -s formatOptions 82 app.png --out site/assets/app.jpg
```
