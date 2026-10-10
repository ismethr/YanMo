# YanMo MVP 功能清单

> **研墨 YanMo**。开源、本地优先、AI 原生的 LaTeX 论文写作与编译桌面应用。
> 架构参考 [PhotoCraft](https://github.com/storytold/photocraft)：底层是 Rust 引擎，所有操作走统一的命令注册表，UI、CLI 和 MCP 共用同一套命令。

## 0. 产品定位与 MVP 目标

**一句话**：把一篇论文当成一个"项目"来管理，包括章节、图表、文献和编译，再让 AI 在完全可控的前提下（每次修改都看 diff、由用户确认）参与写作。

**MVP（v0.1）要验证的三件事**

1. 本地写作闭环够顺：打开 → 编辑 → 自动编译 → PDF 预览 → 双向跳转，体验不输 Overleaf。
2. 结构化管理有价值：大纲、图表、文献三个面板能明显减少"找东西"和"引用出错"。
3. AI 可控、可信：BYOK、多厂商，每次修改都以 diff 形式确认，不会悄悄改动论文。

**MVP 明确不做**：实时多人协作、云同步、网页版、插件市场、Typst/Markdown 支持、所见即所得编辑。

---

## 1. 功能清单

优先级：**P0** 是 v0.1 必须有；**P1** 是 v0.2；**P2** 放进远期池。

### 1.1 项目管理

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| PRJ-1 | 打开文件夹作为项目 | P0 | 项目就是普通文件夹，不使用私有格式；元数据写在 `.yanmo/project.toml` |
| PRJ-2 | 自动识别主文件 | P0 | 扫描含 `\documentclass` 的文件，用户可手动指定 |
| PRJ-3 | 从模板新建 | P0 | 内置 `article`、`ctex-article`、学位论文骨架三套模板 |
| PRJ-4 | 最近项目列表 | P0 | |
| PRJ-5 | 导入 Overleaf zip / 导出 Overleaf zip | P1 | |
| PRJ-6 | 期刊模板库（IEEE、ACM、Elsevier、Springer、高校学位论文） | P1 | |

### 1.2 编辑器

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| EDT-1 | LaTeX 语法高亮、括号匹配、折叠 | P0 | CodeMirror 6 |
| EDT-2 | 命令/环境补全 + 代码片段 | P0 | |
| EDT-3 | `\ref`/`\eqref`/`\cref` 补全 | P0 | 数据来自 label 索引，显示所属图表的标题 |
| EDT-4 | `\cite` 补全 | P0 | 数据来自 bib 索引，显示作者、年份、标题 |
| EDT-5 | 多标签页、文件树、查找替换（支持全项目范围） | P0 | |
| EDT-6 | 自动保存 + 外部修改检测 | P0 | 外部 Agent（如 Claude Code）改了文件时，编辑器能感知并重新加载 |
| EDT-7 | 公式悬停预览 | P1 | KaTeX |
| EDT-8 | 可视化表格编辑器 | P1 | |
| EDT-9 | Vim 键位 | P2 | |

### 1.3 编译与预览

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| CMP-1 | 内置 Tectonic 编译 | P0 | 不需要预装 TeX，宏包按需下载并缓存 |
| CMP-2 | 检测系统 TeX（latexmk + pdf/xe/lualatex） | P0 | 可按项目切换引擎；中文默认用 XeLaTeX |
| CMP-3 | 保存后自动编译（带防抖）、手动编译、取消编译 | P0 | |
| CMP-4 | PDF 预览 | P0 | PDF.js，支持缩放、记忆滚动位置 |
| CMP-5 | SyncTeX 双向跳转 | P0 | |
| CMP-6 | 编译日志结构化 | P0 | 解析为错误/警告列表，点击跳到源码位置 |
| CMP-7 | 编译历史与耗时 | P1 | |
| CMP-8 | 草稿模式（图片占位，加速编译） | P1 | |

### 1.4 论文结构管理 ⭐ 差异化

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| STR-1 | 大纲树 | P0 | 解析 `\part` 到 `\paragraph` 各级标题，自动跟进 `\input`/`\include`，点击跳转 |
| STR-2 | 章节字数统计 | P0 | 去掉命令后的正文字数，中文按字计、英文按词计 |
| STR-3 | 章节状态标记（待写 / 草稿 / 待修改 / 完成） | P0 | 存在 `.yanmo/`，不写进 tex 源码 |
| STR-4 | 交叉引用检查 | P0 | 找出未定义的 ref、重复的 label、从未被引用的 label |
| STR-5 | 大纲中拖拽重排章节 | P1 | 先支持同一文件内，再支持跨文件 |
| STR-6 | 一键把章节拆成独立文件 | P1 | 并自动插入 `\input` |
| STR-7 | 写作进度看板（目标字数、截稿日期） | P2 | |

### 1.5 图表与附件管理 ⭐ 差异化

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| AST-1 | 资产面板 | P0 | 按 figure / table / equation / algorithm 分类，列出 label、caption、所在位置、被引用次数 |
| AST-2 | 图片缩略图 | P0 | 支持 png、jpg、pdf、eps（eps 转换为 P1） |
| AST-3 | 拖入或粘贴图片 | P0 | 自动复制到 `figures/`，并在光标处插入 figure 环境模板 |
| AST-4 | 问题提示 | P0 | 图片文件缺失、文件夹里有图但从未用到、图表从未被引用 |
| AST-5 | 图表脚本绑定 | P1 | 图片关联到生成它的 Python 脚本，可一键重新生成 |
| AST-6 | 补充材料与附件归档 | P2 | 数据、代码、补充材料 |

### 1.6 文献管理 ⭐ 差异化

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| BIB-1 | 文献列表 | P0 | 解析项目内所有 `.bib`，可搜索、排序，查看详情 |
| BIB-2 | 通过 DOI / arXiv ID 一键导入 | P0 | 数据来自 Crossref 和 arXiv API，自动生成 key 并写入 `.bib` |
| BIB-3 | 引用检查 | P0 | 找出 cite 了但不存在的 key、存在但从未被引用的条目、重复条目 |
| BIB-4 | 查看引用位置 | P0 | 查看某条文献在正文哪些位置被引用 |
| BIB-5 | Zotero 集成（Better BibTeX） | P1 | |
| BIB-6 | AI 引用核查 | P1 | 核对文献是否真实存在、元数据是否正确、是否支撑引用它的句子 |
| BIB-7 | 统一 bib 格式（字段顺序、大小写保护） | P1 | |

### 1.7 AI 辅助

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| AI-1 | 多厂商 BYOK | P0 | 支持 Anthropic 和 OpenAI 兼容接口（覆盖 OpenAI、DeepSeek、通义、Ollama 本地模型等）；Key 存在系统钥匙串里 |
| AI-2 | 选中文本快捷操作 | P0 | 润色、学术化改写、中英互译、缩写、扩写，可自定义提示词 |
| AI-3 | 所有修改以 diff 预览 | P0 | 用户可整体或逐块接受/拒绝；修改进入撤销栈 |
| AI-4 | 编译错误解释与修复建议 | P0 | 结合 CMP-6 的结构化日志 |
| AI-5 | 侧栏对话 | P0 | 自动带上当前章节、大纲、相关 label/bib 作为上下文 |
| AI-6 | 生成内容 | P1 | 根据描述生成 LaTeX 表格或公式；截图转 LaTeX（需多模态模型） |
| AI-7 | 整篇审稿 | P1 | 模拟审稿人，按章节给出意见列表 |
| AI-8 | Agent 模式 | P1 | 多步修改：AI 调用命令注册表里的命令，每步都要确认 |
| AI-9 | 用量与费用统计 | P1 | |

### 1.8 命令系统与对外接口（借鉴 PhotoCraft）

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| CMD-1 | 统一命令注册表 | P0 | 每个菜单项、按钮和快捷键都对应一个命令 ID，例如 `compile.run`、`bib.import_doi` |
| CMD-2 | 命令面板（⌘⇧P） | P0 | |
| CMD-3 | CLI：`ym compile`、`ym outline`、`ym check` | P0 | 可在 CI 中检查论文；`check` 输出 ref/cite/图片问题 |
| CMD-4 | MCP Server | P1 | 让 Claude Code、Codex 等外部 Agent 读取大纲、导入文献、编译、读取错误 |
| CMD-5 | 快捷键自定义 | P1 | |

### 1.9 通用

| ID | 功能 | 优先级 | 说明 |
|---|---|---|---|
| GEN-1 | 中英双语界面 | P0 | |
| GEN-2 | 亮色 / 暗色主题 | P0 | |
| GEN-3 | 平台支持 | P0 | macOS（通用二进制）、Windows x64、Linux（AppImage、deb） |
| GEN-4 | 默认不收集任何遥测 | P0 | |
| GEN-5 | Git 面板（状态、提交、diff、推送） | P1 | |
| GEN-6 | 自动更新 | P1 | |

---

## 2. 里程碑

| 里程碑 | 周期 | 内容 | 验收标准 |
|---|---|---|---|
| **M0 骨架** | 第 1–2 周 | workspace、CI、Tauri 空壳、命令注册表、项目打开 | 三平台 CI 构建通过；能打开文件夹并显示文件树 |
| **M1 写作闭环** | 第 3–5 周 | EDT-1/2/5/6，CMP-1~6 | 用 Tectonic 编译示例论文（含中文）；SyncTeX 双向跳转可用；编译错误可点击定位 |
| **M2 结构化** | 第 6–8 周 | `ym-latex` 解析器，STR-1~4，AST-1~4，BIB-1~4，EDT-3/4，CLI | 能检出示例论文里故意埋下的 6 类问题；`ym check` 可在 CI 中运行 |
| **M3 AI** | 第 9–11 周 | AI-1~5 | 接通 Claude、OpenAI 兼容接口和 Ollama；所有 AI 修改都必须经过 diff 确认 |
| **M4 发布 v0.1** | 第 12 周 | 打包签名、文档、示例项目、主页 | 三平台安装包；README 附 GIF 演示；`CONTRIBUTING.md`、`AGENTS.md` 齐全 |

---

## 3. 关键技术决策

| 决策 | 选择 | 理由 |
|---|---|---|
| 桌面框架 | Tauri v2 | Rust 后端可以直接复用 PhotoCraft 式的 crate 分层；安装包小 |
| UI 层 | Web（TypeScript + Svelte 或 React） | CodeMirror 6 和 PDF.js 生态成熟；PhotoCraft 用的 egui 不适合富文本编辑和 PDF 显示 |
| 编辑器 | CodeMirror 6 | 比 Monaco 轻，扩展性好，更方便做 diff 装饰和 inline widget |
| 编译 | Tectonic（Rust 库内嵌）+ 可选 latexmk | 新用户零配置；老用户保留自己的 TeX 环境 |
| LaTeX 解析 | 自研容错解析器（只做结构层面，不做完整宏展开） | 只需要提取大纲、label、ref、cite、includegraphics、input |
| 数据存储 | 源码就是唯一事实来源；`.yanmo/` 只存元数据和缓存 | 不锁定用户，可以随时换回其他编辑器 |
| 许可证 | MIT OR Apache-2.0 | 与 PhotoCraft 一致，对社区友好（注意 Tectonic 是 MIT，可兼容） |
