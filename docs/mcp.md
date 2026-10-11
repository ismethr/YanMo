# 让 Claude Code 等 Agent 接入论文（MCP）

`ym mcp` 在标准输入输出上运行一个 [MCP](https://modelcontextprotocol.io) Server，把研墨的命令注册表开放给 Claude Code、Codex 等编程 Agent。
Agent 可以读取论文大纲与图表、检查引用问题、编译并读取错误、导入文献，和桌面应用里用的是同一套命令。

## 接入

桌面应用里打开论文后，按 ⇧⌘P 运行"接入 Claude Code 等 Agent（复制 MCP 命令）"，会复制一条类似下面的命令，在终端中运行即可：

```bash
claude mcp add yanmo -- /Applications/YanMo.app/Contents/MacOS/ym mcp -C /path/to/paper
```

`ym` 随安装包一起安装（macOS 在 `YanMo.app/Contents/MacOS/`，Windows 在安装目录，Linux deb/rpm 在 `/usr/bin/`）。
使用 AppImage 或只想用命令行时，从 [Releases](https://github.com/ismethr/YanMo/releases) 下载 `ym-<版本>-<平台>` 压缩包。

其他客户端的配置方式相同：命令为 `ym`，参数为 `mcp -C <论文目录>`。可加 `--lang en` 或 `--lang zh` 指定消息语言，`--main <文件>` 指定主文件。

## 工具

| 工具 | 作用 | 改动文件 |
|---|---|---|
| `project_index` | 章节大纲（label、位置、字数）、图表公式、全部 label 与文件 | 否 |
| `check_run` | 检查未定义引用、重复 label、缺失图片、未引用的图表与文献等 | 否 |
| `compile_run` | 编译 PDF，返回带文件与行号的错误和警告 | 生成 `.yanmo/build/` |
| `compile_toolchain` | 可用的编译引擎与工具 | 否 |
| `project_info` / `project_tree` | 主文件、独立文档列表、文件树 | 否 |
| `project_templates` / `project_create` | 列出论文模板；从模板新建论文项目 | 后者新建文件夹 |
| `project_export_zip` / `project_import_zip` | 打包为可上传 Overleaf 的 zip；把 Overleaf 的 zip 解压为本地项目 | 写入 zip / 新建文件夹 |
| `project_open` / `project_set_main` | 打开其他论文目录；切换主文件 | 后者写 `.yanmo/project.toml` |
| `outline_move` | 把章节（连同子节）移到同级章节之前或之后；章节各自成文件时调整 `\input` 顺序 | 是 |
| `outline_extract` | 把章节拆成独立文件，原处换成 `\input`（编译结果不变） | 是 |
| `bib_fetch` | 按 DOI / arXiv 查询 BibTeX（联网） | 否 |
| `zotero_search` / `zotero_add` | 搜索本机 Zotero 文献库；把选中的条目加入项目 .bib（去掉附件路径、笔记、摘要，重新生成 key） | 后者是 |
| `bib_verify` | 到 Crossref / doi.org / arXiv 核对文献是否存在、信息是否一致，并建议缺失的 DOI（联网） | 否 |
| `bib_abstracts` | 获取被引文献的摘要（Semantic Scholar，备选 Crossref），用于判断引用是否支撑论述（联网） | 否 |
| `bib_add_field` | 给条目补充缺少的字段（如 DOI），只插入一行 | 是 |
| `bib_import` / `bib_add` | 把文献加入项目的 .bib 文件 | 是 |
| `asset_import` | 把图片复制到项目图片目录 | 是 |
| `export_docx` | 用 Pandoc 导出 Word | 写入 `.yanmo/export/` |
| `synctex_forward` / `synctex_inverse` | 源码行与 PDF 位置互相定位 | 否 |

改稿本身交给 Agent 自己的文件编辑工具，并由你在 Agent 中确认；研墨不开放任意写文件的命令。
会改动文件的工具都标注为非只读，客户端会按自己的权限设置请你确认。每次调用都会重新读取磁盘上的源码，Agent 改完文件后再检查即可看到最新结果。
