<p align="center"><img src="assets/brand/icon-1024.png" width="128" alt="YanMo 图标"></p>

# 研墨 YanMo

开源、本地优先、AI 原生的 LaTeX 论文写作与编译桌面应用。

- **写作闭环**：CodeMirror 编辑器 + 内置 Tectonic 编译（零配置）+ PDF 预览 + SyncTeX 双向跳转
- **结构化管理**：大纲与章节进度、图表资产库、文献库，自动发现引用问题
- **可控的 AI**：自带 API Key（Claude / OpenAI 兼容 / Ollama），每次修改都以 diff 呈现、经确认后应用
- **对 Agent 友好**：统一命令注册表，CLI 与 MCP Server 和 UI 共用同一套命令

> 状态：pre-alpha。路线图见 [docs/MVP.md](docs/MVP.md)，架构见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 开发

依赖：Rust stable、Node 22、pnpm（版本见 `apps/desktop/ui/package.json` 的 `packageManager`）；Linux 另需 webkit2gtk-4.1。

```bash
pnpm --dir apps/desktop/ui install

# 桌面应用（Tauri，会自动启动前端开发服务器）
pnpm --dir apps/desktop/ui tauri dev

# 只改界面：浏览器预览模式，使用 examples/sample-paper 的示例数据，无需编译 Rust
pnpm --dir apps/desktop/ui dev

# 测试与命令行
cargo test --workspace
cargo run -p ym-cli -- -C examples/sample-paper check
cargo run -p ym-cli -- exec                      # 列出命令注册表中的所有命令
```

改动了 Rust 命令的返回结构后，运行 `pnpm --dir apps/desktop/ui fixtures` 重新生成预览模式的数据。

## 许可证

可在 [MIT](LICENSE-MIT) 与 [Apache-2.0](LICENSE-APACHE) 之间任选其一。
