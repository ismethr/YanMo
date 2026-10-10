// 前端唯一的后端调用入口，与 Rust 侧 ym-commands 注册表一一对应。
//
// 在 Tauri 中运行时走 invoke；直接用浏览器打开 `pnpm dev` 时走预览模式，
// 数据来自 examples/sample-paper 生成的 fixtures（见 scripts/gen-fixtures.sh），便于只改界面的贡献者。

import { invoke } from '@tauri-apps/api/core';
import type { CommandMeta } from './types';
import info from './fixtures/project.info.json';
import tree from './fixtures/project.tree.json';
import index from './fixtures/project.index.json';
import check from './fixtures/check.run.json';
import files from './fixtures/files.json';
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
    { id: 'project.open', title: '打开文件夹', mutating: false },
    { id: 'check.run', title: '检查项目问题', mutating: false },
    { id: 'file.write', title: '写入文件', mutating: true },
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

/** 弹出系统对话框选择论文文件夹；预览模式直接返回示例论文。 */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return info.root;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ directory: true, multiple: false, title: '打开论文文件夹' });
  return typeof picked === 'string' ? picked : null;
}

const previewFiles: Record<string, string> = { ...files };

function preview(id: string, params: Record<string, unknown>): unknown {
  const path = params.path as string;
  switch (id) {
    case 'project.open':
    case 'project.info':
      return info;
    case 'project.tree':
      return tree;
    case 'project.index':
      return index;
    case 'check.run':
      return check;
    case 'file.read':
      if (!(path in previewFiles)) throw new Error(`预览模式中没有文件：${path}`);
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
    case 'export.docx':
      throw new Error('浏览器预览模式不支持导出 Word，请在桌面应用中使用');
    case 'synctex.forward':
    case 'synctex.inverse':
      return null; // 预览模式没有 SyncTeX 数据
    default:
      throw new Error(`未知命令：${id}`);
  }
}
