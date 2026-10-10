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

### 安装 cloudflared（Ubuntu / Debian）

```bash
sudo mkdir -p --mode=0755 /usr/share/keyrings
curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg | sudo tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main" \
  | sudo tee /etc/apt/sources.list.d/cloudflared.list
sudo apt-get update && sudo apt-get install -y cloudflared
```

如果这台机器上运行着透明代理（如 mihomo），出站 UDP 经代理不一定可用，建议固定使用 HTTP/2：

```bash
sudo mkdir -p /etc/systemd/system/cloudflared.service.d
printf '[Service]\nEnvironment=TUNNEL_TRANSPORT_PROTOCOL=http2\n' \
  | sudo tee /etc/systemd/system/cloudflared.service.d/yanmo.conf
sudo systemctl daemon-reload
```

### 创建隧道并注册服务

1. 域名先接入 Cloudflare（在注册商处把 DNS 服务器改为 Cloudflare 分配的地址，等待站点显示为已激活）。
2. Zero Trust → Networks → Tunnels → Create a tunnel，类型选 Cloudflared，页面会给出带令牌的安装命令：
   `sudo cloudflared service install <令牌>`。令牌等同于隧道密码，只在服务器上运行，不要提交到仓库。
3. 运行后用 `systemctl status cloudflared` 确认服务在运行，隧道页面显示 HEALTHY。

### 公网主机名

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
