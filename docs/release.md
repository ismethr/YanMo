# 发布流程

## 发布一个版本

1. 更新版本号（三处保持一致）：
   - `Cargo.toml` 中 `[workspace.package] version`
   - `apps/desktop/src-tauri/tauri.conf.json` 中 `version`
   - `apps/desktop/ui/package.json` 中 `version`
2. 在 `CHANGELOG.md` 中补充本版本的变化，提交并推送到 `main`，确认 CI 通过。
3. 打标签并推送，触发 `.github/workflows/release.yml`：

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. 流水线在 macOS（Apple 芯片 / Intel）、Windows、Linux 上构建安装包，上传到同名的 **GitHub Release 草稿**。
   在 Releases 页面检查各平台的安装包能否正常安装、打开、编译示例论文，然后点击 **Publish release**。

也可以在 Actions 页面手动运行 Release 工作流（workflow_dispatch），用于试构建。

## 本地打包

```bash
sh scripts/fetch-tectonic.sh --sidecar          # 下载本机平台的 Tectonic 到 src-tauri/binaries/
pnpm --dir apps/desktop/ui install
pnpm --dir apps/desktop/ui tauri build --config ../src-tauri/tauri.release.conf.json
```

产物在 `target/release/bundle/`。`tauri.release.conf.json` 只在打包时使用：它把 Tectonic 作为 sidecar 打进安装包，
并为 macOS 应用做 ad-hoc 签名。日常开发与 CI 不需要下载 Tectonic。

## 内置的 Tectonic

安装包里的 Tectonic 与 YanMo 放在同一目录（macOS 为 `YanMo.app/Contents/MacOS/tectonic`）。YanMo 查找编译引擎的顺序：
环境变量 `YANMO_TECTONIC` → 程序同目录 → `~/.yanmo/bin` → `PATH`。首次编译时 Tectonic 会联网下载所需宏包并缓存。

## 代码签名（可选）

未配置签名时：

- **macOS**：应用为 ad-hoc 签名，首次打开会提示"无法验证开发者"。用户需在"访达"中右键选择"打开"，
  或运行 `xattr -cr /Applications/YanMo.app`。
- **Windows**：SmartScreen 可能提示"未知发布者"，选择"仍要运行"。

### macOS 签名与公证

需要 Apple Developer 账号。在仓库 Settings → Secrets and variables → Actions 中添加：

| Secret | 内容 |
|---|---|
| `APPLE_CERTIFICATE` | "Developer ID Application" 证书导出的 .p12，base64 编码 |
| `APPLE_CERTIFICATE_PASSWORD` | .p12 的密码 |
| `APPLE_SIGNING_IDENTITY` | 证书名称，如 `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | 用于公证的 Apple ID、App 专用密码与团队 ID |

然后在 `.github/workflows/release.yml` 的 tauri-action 步骤 `env:` 中加入：

```yaml
APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
APPLE_ID: ${{ secrets.APPLE_ID }}
APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
```

（没有配置 secret 时不要加：空字符串会被当作证书去导入，导致打包失败。）`APPLE_SIGNING_IDENTITY` 会覆盖
`tauri.release.conf.json` 中的 ad-hoc 签名。

### Windows 签名

参见 Tauri 文档的 Windows Code Signing 一节，配置证书后在 `tauri.release.conf.json` 的 `bundle.windows` 中填写签名信息。
