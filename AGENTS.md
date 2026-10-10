# AGENTS.md — 给 AI 编码助手的项目约定

## 分层（禁止反向依赖）
```
ym-model  ←  ym-latex, ym-bib  ←  ym-check, ym-compile, ym-ai  ←  ym-commands  ←  apps/*
```
- `ym-model` 只放纯数据类型，不做网络或进程调用。
- 新功能 = 在 `ym-commands` 注册一个命令；不要在 `apps/desktop` 里直接写业务逻辑。
- 前端只通过 `execute(id, params)` 访问后端。

## 不可违反的规则
- AI 只能产出 `ym_ai::Patch`，**禁止直接写用户文件**。
- tex/bib 源码是唯一事实来源；`.yanmo/` 只放元数据和缓存。
- 写回 `.bib`/`.tex` 时必须做最小化 diff，保留用户原有格式。
- 解析器面对任何输入都不能 panic（需要 fuzz 测试）。

## 提交前
`cargo fmt --all && cargo clippy --workspace -- -D warnings && cargo test --workspace`
