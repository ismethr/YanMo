<p align="center"><img src="assets/brand/icon-1024.png" width="128" alt="YanMo 图标"></p>

# 研墨 YanMo

开源、本地优先、AI 原生的 LaTeX 论文写作与编译桌面应用。

- **写作闭环**：CodeMirror 编辑器 + 内置 Tectonic 编译（零配置）+ PDF 预览 + SyncTeX 双向跳转
- **结构化管理**：大纲与章节进度、图表资产库、文献库，自动发现引用问题
- **可控的 AI**：自带 API Key（Claude / OpenAI 兼容 / Ollama），每次修改都以 diff 呈现、经确认后应用
- **对 Agent 友好**：统一命令注册表，CLI 与 MCP Server 和 UI 共用同一套命令

> 状态：预览版 v0.2。官网 [yanmo.ink](https://yanmo.ink)；路线图见 [docs/MVP.md](docs/MVP.md)，架构见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)，更新见 [CHANGELOG](CHANGELOG.md)。

## 安装

从 [Releases](https://github.com/ismethr/YanMo/releases) 下载对应系统的安装包（macOS .dmg、Windows .exe / .msi、Linux .AppImage / .deb）。
安装包已内置 Tectonic 编译引擎，不需要另外安装 TeX；导出 Word 需要另外安装 [Pandoc](https://pandoc.org/installing.html)。

macOS 首次打开如提示"无法验证开发者"：在"访达"中右键点击 YanMo 选择"打开"，或运行 `xattr -cr /Applications/YanMo.app`。

界面支持中文与英文，在状态栏右下角切换。

## 接入 Claude Code 等 Agent

安装包附带 `ym` 命令行，其中的 `ym mcp` 是 MCP Server：Agent 可以读取大纲、检查引用、编译并读取错误、导入文献。接入方法见 [docs/mcp.md](docs/mcp.md)。

## 开发

依赖：Rust stable、Node 22、pnpm（版本见 `apps/desktop/ui/package.json` 的 `packageManager`）；Linux 另需 webkit2gtk-4.1。

```bash
pnpm --dir apps/desktop/ui install

# 桌面应用（Tauri，会自动启动前端开发服务器）；需要先运行一次 sh scripts/fetch-tectonic.sh
pnpm --dir apps/desktop/ui tauri dev

# 只改界面：浏览器预览模式，使用 examples/sample-paper 的示例数据，无需编译 Rust
pnpm --dir apps/desktop/ui dev

# 测试与命令行
cargo test --workspace
cargo run -p ym-cli -- -C examples/sample-paper check
cargo run -p ym-cli -- exec                      # 列出命令注册表中的所有命令
```

打包发布见 [docs/release.md](docs/release.md)。

改动了 Rust 命令的返回结构后，运行 `pnpm --dir apps/desktop/ui fixtures` 重新生成预览模式的数据。

## 许可证

可在 [MIT](LICENSE-MIT) 与 [Apache-2.0](LICENSE-APACHE) 之间任选其一。
