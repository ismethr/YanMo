// 应用状态（Svelte 5 runes）。所有对项目的读写都经由 backend.execute，即 Rust 命令注册表。

import { execute, pickFolder } from './backend';
import type { CheckResult, Issue, ProjectIndex, ProjectInfo, TreeNode } from './types';

export type Activity = 'files' | 'outline' | 'assets' | 'bib';

/** PDF 预览标签页的固定 ID（与真实文件路径区分开）。 */
export const PDF_TAB = 'yanmo://pdf';

interface Doc {
  text: string;
  saved: string;
}

const AUTOSAVE_MS = 800;
const RECENT_KEY = 'yanmo.recent';

function loadRecent(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
  } catch {
    return [];
  }
}

class AppState {
  project = $state<ProjectInfo | null>(null);
  tree = $state<TreeNode[]>([]);
  index = $state<ProjectIndex | null>(null);
  check = $state<CheckResult | null>(null);
  activity = $state<Activity>('outline');
  tabs = $state<string[]>([]);
  active = $state<string | null>(null);
  docs = $state<Record<string, Doc>>({});
  bottom = $state<'problems' | 'log'>('problems');
  aiOpen = $state(false);
  palette = $state(false);
  cursor = $state({ line: 1, col: 1 });
  /** 编辑器跳转请求；n 递增以便重复跳到同一行 */
  reveal = $state<{ path: string; line: number; n: number } | null>(null);
  notice = $state<string | null>(null);
  recent = $state<string[]>(loadRecent());

  #timers = new Map<string, ReturnType<typeof setTimeout>>();

  get errors() {
    return this.check?.errors ?? 0;
  }

  get warnings() {
    return this.check?.warnings ?? 0;
  }

  isDirty(path: string) {
    const d = this.docs[path];
    return !!d && d.text !== d.saved;
  }

  issuesFor(path: string): Issue[] {
    return this.check?.issues.filter((i) => i.file === path) ?? [];
  }

  flash(msg: string) {
    this.notice = msg;
    setTimeout(() => {
      if (this.notice === msg) this.notice = null;
    }, 4000);
  }

  async openFolder(path?: string) {
    const target = path ?? (await pickFolder());
    if (!target) return;
    try {
      this.project = await execute<ProjectInfo>('project.open', { path: target });
    } catch (e) {
      this.flash(`无法打开：${e}`);
      return;
    }
    this.recent = [target, ...this.recent.filter((r) => r !== target)].slice(0, 8);
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(this.recent));
    } catch {
      /* 存储不可用时只是不记最近项目 */
    }
    this.tabs = [];
    this.docs = {};
    this.active = null;
    await this.refresh();
    if (this.project.main) await this.openFile(this.project.main);
  }

  /** 重新读取文件树、索引与检查结果（保存后调用）。 */
  async refresh() {
    const [tree, index, check] = await Promise.allSettled([
      execute<TreeNode[]>('project.tree'),
      execute<ProjectIndex>('project.index'),
      execute<CheckResult>('check.run'),
    ]);
    if (tree.status === 'fulfilled') this.tree = tree.value;
    this.index = index.status === 'fulfilled' ? index.value : null;
    this.check = check.status === 'fulfilled' ? check.value : null;
    if (index.status === 'rejected') this.flash(String(index.reason));
  }

  async openFile(path: string, line?: number) {
    if (path !== PDF_TAB && !(path in this.docs)) {
      try {
        const { text } = await execute<{ text: string }>('file.read', { path });
        this.docs[path] = { text, saved: text };
      } catch (e) {
        this.flash(String(e));
        return;
      }
    }
    if (!this.tabs.includes(path)) this.tabs.push(path);
    this.active = path;
    if (line) this.reveal = { path, line, n: (this.reveal?.n ?? 0) + 1 };
  }

  closeTab(path: string) {
    const i = this.tabs.indexOf(path);
    if (i < 0) return;
    if (this.isDirty(path)) void this.save(path);
    this.tabs.splice(i, 1);
    if (this.active === path) this.active = this.tabs[Math.min(i, this.tabs.length - 1)] ?? null;
  }

  edit(path: string, text: string) {
    const doc = this.docs[path];
    if (!doc) return;
    doc.text = text;
    clearTimeout(this.#timers.get(path));
    this.#timers.set(path, setTimeout(() => void this.save(path), AUTOSAVE_MS));
  }

  async save(path: string) {
    const doc = this.docs[path];
    if (!doc || doc.text === doc.saved) return;
    clearTimeout(this.#timers.get(path));
    const text = doc.text;
    try {
      await execute('file.write', { path, text });
      doc.saved = text;
      await this.refresh();
    } catch (e) {
      this.flash(`保存失败：${e}`);
    }
  }

  async saveActive() {
    if (this.active && this.active !== PDF_TAB) await this.save(this.active);
  }
}

export const app = new AppState();
