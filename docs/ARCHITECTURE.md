# 架构概览

```
┌───────────────────────── apps/desktop (Tauri v2) ─────────────────────────┐
│  ui/ (TypeScript)                                                          │
│  文件树 │ 大纲 │ 资产 │ 文献 ║ CodeMirror 编辑器 ║ PDF.js 预览 ║ AI 面板     │
│                         │  execute(id, params)                            │
├─────────────────────────┼──────────────────────────────────────────────────┤
│  src-tauri/bridge.rs ───┘                                                  │
└─────────────────────────┬──────────────────────────────────────────────────┘
          apps/cli (ym) ──┤── apps/mcp (stdio MCP)
                          ▼
                 ym-commands（统一命令注册表）
        ┌──────────┬──────┴─────┬───────────┬──────────┐
   ym-compile    ym-ai      ym-check     ym-bib     ym-latex
   Tectonic/    多厂商LLM   一致性检查   bib解析/   结构级解析
   latexmk/     Patch生成                DOI导入
   SyncTeX
        └──────────┴────────────┴───────────┴──────────┘
                          ym-model（纯数据）
```

## 关键数据流

**编辑 → 编译**：文件保存 → 防抖 300ms → `compile.run` → `ym-compile` → `CompileOutput`（PDF + 诊断）→ 刷新 PDF 预览和问题面板。

**索引**：文件变化 → `ym-latex` 增量重解析该文件 → 合并项目索引 → 刷新大纲、资产、文献面板和补全数据 → `ym-check` 更新问题列表。

**AI 修改**：用户操作 → `ym-ai` 组装上下文 → 调用 Provider → 生成 `Patch` → 前端展示 diff → 用户接受 → `edit.apply_patch` 命令 → 写文件并进入撤销栈。

## 与 PhotoCraft 的对应关系

| PhotoCraft | Yanmo |
|---|---|
| 纯数据文档模型 + 命令引擎 | `ym-model` + `ym-commands` |
| 500+ 命令同时服务 UI、CLI、JSON 通道、MCP | 同上，命令 ID 形如 `compile.run` |
| 编译期强制 crate 分层 | 同上（见 `AGENTS.md`） |
| egui 原生 UI | **改用** Tauri + Web UI（编辑器和 PDF 生态更成熟） |
| 独立的 `photocraft-psd` crate | `ym-bib`、`ym-latex` 可独立发布 |
