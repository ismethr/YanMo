// 前端唯一的后端调用入口，与 Rust 侧 ym-commands 注册表一一对应。
//
// 在 Tauri 中运行时走 invoke；直接用浏览器打开 `pnpm dev` 时走预览模式，
// 数据来自示例论文生成的 fixtures（见 scripts/gen-fixtures.sh），便于只改界面的贡献者：
// 中文界面用 examples/sample-paper，英文界面用 examples/sample-paper-en。

import { invoke } from '@tauri-apps/api/core';
import type { CommandMeta } from './types';
import info from './fixtures/project.info.json';
import tree from './fixtures/project.tree.json';
import index from './fixtures/project.index.json';
import check from './fixtures/check.run.json';
import files from './fixtures/files.json';
import infoEn from './fixtures/en/project.info.json';
import treeEn from './fixtures/en/project.tree.json';
import indexEn from './fixtures/en/project.index.json';
import checkEn from './fixtures/en/check.run.json';
import filesEn from './fixtures/en/files.json';
import { i18n, tr } from './i18n.svelte';
import compileRun from './fixtures/compile.run.json';
import compilePdf from './fixtures/compile.pdf.json';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function execute<T>(id: string, params: Record<string, unknown> = {}): Promise<T> {
  if (inTauri) return invoke<T>('execute', { id, params });
  return preview(id, params) as T;
}

export async function listCommands(): Promise<CommandMeta[]> {
  if (inTauri) return invoke<CommandMeta[]>('list_commands');
  return [
    { id: 'project.open', title: '打开文件夹', title_en: 'Open Folder', mutating: false },
    { id: 'check.run', title: '检查项目问题', title_en: 'Check Project', mutating: false },
    { id: 'file.write', title: '写入文件', title_en: 'Write File', mutating: true },
  ];
}

/** 在访达 / 资源管理器中显示文件（仅桌面端）。 */
export async function revealInFolder(absPath: string): Promise<void> {
  if (!inTauri) return;
  const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
  await revealItemInDir(absPath);
}

/** 命令行启动参数中的项目文件夹（仅桌面端）。 */
export async function launchFolder(): Promise<string | null> {
  return inTauri ? invoke<string | null>('launch_folder') : null;
}

/** 随应用安装的 ym 命令行路径；预览模式与找不到时返回 null（假定 ym 在 PATH 中）。 */
export async function cliPath(): Promise<string | null> {
  return inTauri ? invoke<string | null>('cli_path') : null;
}

/** 弹出系统对话框选择论文文件夹；预览模式直接返回示例论文。 */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return fixtures().info.root;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ directory: true, multiple: false, title: tr('打开论文文件夹', 'Open Paper Folder') });
  return typeof picked === 'string' ? picked : null;
}

// 预览模式的示例论文按首次使用时的界面语言选择；大纲与文件内容可以在内存中被修改
let preview_: { info: typeof info; tree: typeof tree; index: typeof index; check: typeof check; files: Record<string, string> } | null = null;
function fixtures() {
  if (!preview_) {
    const en = i18n.lang === 'en';
    preview_ = {
      info: en ? infoEn : info,
      tree: en ? treeEn : tree,
      index: structuredClone(en ? indexEn : index) as typeof index,
      check: (en ? checkEn : check) as typeof check,
      files: { ...(en ? filesEn : files) },
    };
  }
  return preview_;
}

/** 预览模式的移动章节：只调整内存中大纲的顺序 */
function previewMove(id: string, target: string, position: string): boolean {
  type Node = { id: string; children: Node[] };
  const visit = (list: Node[]): boolean => {
    const i = list.findIndex((s) => s.id === id);
    const j = list.findIndex((s) => s.id === target);
    if (i >= 0 && j >= 0) {
      const [s] = list.splice(i, 1);
      const k = list.findIndex((x) => x.id === target);
      list.splice(position === 'before' ? k : k + 1, 0, s);
      return true;
    }
    return list.some((s) => visit(s.children));
  };
  return visit(fixtures().index.outline as Node[]);
}

function preview(id: string, params: Record<string, unknown>): unknown {
  const path = params.path as string;
  const { info, tree, index, check, files: previewFiles } = fixtures();
  switch (id) {
    case 'project.open':
    case 'project.info':
      return info;
    case 'project.tree':
      return tree;
    case 'project.index':
      return index;
    case 'outline.move':
      if (!previewMove(params.id as string, params.target as string, params.position as string))
        throw new Error(tr('只能在同级章节之间移动', 'Sections can only be moved among sections of the same level'));
      return { file: 'main.tex', changed: true };
    case 'check.run':
      return check;
    case 'app.set_lang':
      return { lang: params.lang };
    case 'file.read':
      if (!(path in previewFiles)) throw new Error(tr(`预览模式中没有文件：${path}`, `No such file in preview mode: ${path}`));
      return { path, text: previewFiles[path] };
    case 'file.write':
      previewFiles[path] = params.text as string;
      return { path };
    case 'compile.run':
      return compileRun;
    case 'compile.pdf':
      return compilePdf;
    case 'compile.toolchain':
      return { tectonic: 'tectonic', latexmk: null, pandoc: 'pandoc', ghostscript: null };
    case 'project.set_main':
      return { ...info, main: path };
    case 'bib.verify':
      return {
        results: [
          { key: 'vaswani2017attention', status: 'not_found', notes: [tr('未在 Crossref 找到对应记录（书籍、报告、网页常见此情况），请人工确认', 'No matching Crossref record (common for books, reports and web pages); please check it by hand')], found: null, suggested_doi: null },
          { key: 'unused2020', status: 'mismatch', notes: [tr('年份不一致：条目为 2020，登记为 2017', 'Year differs: 2020 in the entry, 2017 registered')], found: null, suggested_doi: null },
        ],
      };
    case 'outline.extract':
      throw new Error(tr('浏览器预览模式不支持拆分文件，请在桌面应用中使用', 'Moving a section to a file is not available in the browser preview; use the desktop app'));
    case 'export.docx':
      throw new Error(tr('浏览器预览模式不支持导出 Word，请在桌面应用中使用', 'Word export is not available in the browser preview; use the desktop app'));
    case 'bib.fetch':
      return {
        key: 'he2016deep',
        title: 'Deep Residual Learning for Image Recognition',
        bibtex: '@inproceedings{he2016deep,\n  author = {He, Kaiming and others},\n  title = {Deep Residual Learning for Image Recognition},\n  year = {2016},\n}\n',
      };
    case 'bib.add':
      return { key: 'he2016deep', duplicate: false, file: 'refs.bib' };
    case 'asset.import':
      return { path: 'figures/pasted.png', include: 'figures/pasted', label: 'fig:pasted' };
    case 'synctex.forward':
    case 'synctex.inverse':
      return null; // 预览模式没有 SyncTeX 数据
    default:
      throw new Error(tr(`未知命令：${id}`, `Unknown command: ${id}`));
  }
}
