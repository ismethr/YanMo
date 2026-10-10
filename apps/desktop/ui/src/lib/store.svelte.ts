// 应用状态（Svelte 5 runes）。所有对项目的读写都经由 backend.execute，即 Rust 命令注册表。

import { execute, pickFolder, revealInFolder } from './backend';
import { setLang, tr, type Lang } from './i18n.svelte';
import type {
  CheckResult,
  CompileOutput,
  ExportOutput,
  Section,
  Issue,
  PdfRect,
  ProjectIndex,
  ProjectInfo,
  SourceLocation,
  Toolchain,
  TreeNode,
} from './types';

export type Activity = 'files' | 'outline' | 'assets' | 'bib';

/** PDF 预览标签页的固定 ID（与真实文件路径区分开）。 */
export const PDF_TAB = 'yanmo://pdf';
/** 图片预览标签页的 ID 前缀 */
export const IMAGE_TAB = 'yanmo://image/';

interface Doc {
  text: string;
  saved: string;
}

const AUTOSAVE_MS = 800;
const RECENT_KEY = 'yanmo.recent';
const LAYOUT_KEY = 'yanmo.layout';

interface Layout {
  sidebarWidth: number;
  panelHeight: number;
  panelOpen: boolean;
}

function loadLayout(): Layout {
  const fallback = { sidebarWidth: 272, panelHeight: 200, panelOpen: true };
  try {
    return { ...fallback, ...JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? '{}') };
  } catch {
    return fallback;
  }
}

/** 带编号与聚合字数的大纲节点 */
export interface OutlineRow {
  id: string;
  number: string;
  title: string;
  depth: number;
  file: string;
  line: number;
  /** 本节及所有小节的字数 */
  total: number;
  hasChildren: boolean;
  parents: string[];
}

function flattenOutline(sections: Section[], depth = 0, prefix = '', parents: string[] = []): OutlineRow[] {
  const rows: OutlineRow[] = [];
  let n = 0;
  for (const s of sections) {
    const number = s.numbered ? `${prefix}${++n}` : '';
    const sum = (x: Section): number => x.words + x.children.reduce((a, c) => a + sum(c), 0);
    rows.push({
      id: s.id,
      number,
      title: s.title,
      depth,
      file: s.span.file,
      line: s.span.line,
      total: sum(s),
      hasChildren: s.children.length > 0,
      parents,
    });
    rows.push(...flattenOutline(s.children, depth + 1, number ? `${number}.` : prefix, [...parents, s.id]));
  }
  return rows;
}

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
  notice = $state<{ text: string; action?: { label: string; run: () => void } } | null>(null);
  recent = $state<string[]>(loadRecent());

  // 布局（按用户记住）
  layout = $state<Layout>(loadLayout());

  /** 打开底部面板并切到指定页 */
  showPanel(tab: 'problems' | 'log') {
    this.bottom = tab;
    if (!this.layout.panelOpen) {
      this.layout.panelOpen = true;
      this.saveLayout();
    }
  }

  togglePanel() {
    this.layout.panelOpen = !this.layout.panelOpen;
    this.saveLayout();
  }

  saveLayout() {
    try {
      localStorage.setItem(LAYOUT_KEY, JSON.stringify(this.layout));
    } catch {
      /* 存储不可用时只是不记住布局 */
    }
  }

  /** 扁平化的大纲：章节编号、层级、本节及小节的总字数 */
  outlineRows = $derived(this.index ? flattenOutline(this.index.outline) : []);

  /** 光标所在的章节 */
  get currentSectionId(): string | null {
    let found: string | null = null;
    for (const r of this.outlineRows) {
      if (r.file === this.active && r.line <= this.cursor.line) found = r.id;
    }
    return found;
  }

  /** 中文论文按"字"，英文按"词" */
  get wordUnit(): string {
    const cjk = this.outlineRows.some((r) => /[\u4e00-\u9fff]/.test(r.title));
    return cjk ? tr('字', 'chars') : tr('词', 'words');
  }

  // 编译与 PDF
  toolchain = $state<Toolchain | null>(null);
  compiling = $state(false);
  lastCompile = $state<CompileOutput | null>(null);
  /** 当前 PDF 的字节；rev 递增表示有新版本 */
  pdf = $state<{ data: Uint8Array; rev: number } | null>(null);
  /** 正向同步的目标区域；n 递增以便重复定位 */
  syncTarget = $state<(PdfRect & { n: number }) | null>(null);
  autoCompile = $state(true);
  exporting = $state(false);
  /** 请求编辑器在光标处插入文本；n 递增以区分多次请求 */
  insertRequest = $state<{ path: string; text: string; n: number } | null>(null);
  /** 编辑器当前选区（只记录文本标签页） */
  selection = $state<{ path: string; from: number; to: number; text: string } | null>(null);
  /** 请求编辑器替换一段文本；编辑器校验原文未变后执行，并通过 resolve 返回结果 */
  replaceRequest = $state<{
    path: string;
    from: number;
    to: number;
    expected: string;
    text: string;
    n: number;
    resolve: (ok: boolean) => void;
  } | null>(null);
  importingBib = $state(false);
  lastExport = $state<ExportOutput | null>(null);
  #compileAgain = false;

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

  flash(text: string, action?: { label: string; run: () => void }) {
    const notice = { text, action };
    this.notice = notice;
    setTimeout(() => {
      if (this.notice === notice) this.notice = null;
    }, action ? 8000 : 4000);
  }

  async openFolder(path?: string) {
    const target = path ?? (await pickFolder());
    if (!target) return;
    try {
      this.project = await execute<ProjectInfo>('project.open', { path: target });
    } catch (e) {
      this.flash(tr(`无法打开：${e}`, `Cannot open: ${e}`));
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
    this.lastCompile = null;
    this.pdf = null;
    this.syncTarget = null;
    await this.refresh();
    if (this.project.main) await this.openFile(this.project.main);
    execute<Toolchain>('compile.toolchain').then((t) => (this.toolchain = t), () => {});
    // 沿用上次编译留下的 PDF（如果有）
    await this.loadPdf(true);
  }

  /** 切换界面语言；检查结果等由后端生成的文字需要重新拉取。 */
  async switchLang(lang: Lang) {
    await setLang(lang);
    if (this.project) await this.refresh();
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
    if (/\.(png|jpe?g|gif|svg|pdf|eps)$/i.test(path) && !path.startsWith('yanmo://')) {
      path = IMAGE_TAB + path;
    }
    if (!path.startsWith('yanmo://') && !(path in this.docs)) {
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
      if (this.autoCompile) void this.compile();
    } catch (e) {
      this.flash(tr(`保存失败：${e}`, `Save failed: ${e}`));
    }
  }

  async saveActive() {
    if (this.active && !this.active.startsWith('yanmo://')) await this.save(this.active);
  }

  /** 编译；编译进行中再次触发时，结束后补编一次（连续保存只多编一次）。 */
  async compile() {
    if (!this.project) return;
    if (this.compiling) {
      this.#compileAgain = true;
      return;
    }
    this.compiling = true;
    try {
      const out = await execute<CompileOutput>('compile.run');
      this.lastCompile = out;
      if (out.pdf) await this.loadPdf();
      if (!out.success) {
        const n = out.diagnostics.filter((d) => d.severity === 'error').length;
        this.flash(
          out.pdf
            ? tr(`编译完成，但有 ${n} 个错误`, `Compiled with ${n} error${n === 1 ? '' : 's'}`)
            : tr('编译失败，详见编译日志', 'Compilation failed, see the build log'),
        );
      }
    } catch (e) {
      this.flash(tr(`无法编译：${e}`, `Cannot compile: ${e}`));
    } finally {
      this.compiling = false;
    }
    if (this.#compileAgain) {
      this.#compileAgain = false;
      void this.compile();
    }
  }

  async loadPdf(quiet = false) {
    try {
      const { base64 } = await execute<{ base64: string }>('compile.pdf');
      const bin = atob(base64);
      const data = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) data[i] = bin.charCodeAt(i);
      this.pdf = { data, rev: (this.pdf?.rev ?? 0) + 1 };
    } catch (e) {
      if (!quiet) this.flash(String(e));
    }
  }

  /** 源码 → PDF：在 PDF 中高亮当前行。 */
  async forwardSync(path = this.active, line = this.cursor.line) {
    if (!path || path.startsWith('yanmo://')) return;
    if (!this.pdf) {
      this.flash(tr('还没有 PDF，请先编译（⌘B）', 'No PDF yet. Compile first (⌘B)'));
      return;
    }
    try {
      const rect = await execute<PdfRect | null>('synctex.forward', { path, line });
      if (!rect) {
        this.flash(tr('这一行在 PDF 中没有对应位置', 'This line has no position in the PDF'));
        return;
      }
      await this.openFile(PDF_TAB);
      this.syncTarget = { ...rect, n: (this.syncTarget?.n ?? 0) + 1 };
    } catch (e) {
      this.flash(String(e));
    }
  }

  /** PDF → 源码：跳到点击位置对应的源码行。坐标为 PDF 点，原点在页面左上角。 */
  async inverseSync(page: number, x: number, y: number) {
    try {
      const loc = await execute<SourceLocation | null>('synctex.inverse', { page, x, y });
      if (loc) await this.openFile(loc.file, loc.line);
    } catch (e) {
      this.flash(String(e));
    }
  }

  /** 切换主文件（写入 .yanmo/project.toml），并重新索引、清空旧的编译结果。 */
  async setMain(path: string) {
    if (!this.project || path === this.project.main) return;
    try {
      this.project = await execute<ProjectInfo>('project.set_main', { path });
    } catch (e) {
      this.flash(String(e));
      return;
    }
    this.lastCompile = null;
    this.pdf = null;
    this.syncTarget = null;
    await this.refresh();
    await this.openFile(path);
    await this.loadPdf(true);
  }

  /** 用 Pandoc 导出 Word。先编译过才能把交叉引用替换为编号。 */
  async exportDocx() {
    if (!this.project || this.exporting) return;
    this.exporting = true;
    try {
      const out = await execute<ExportOutput>('export.docx');
      this.lastExport = out;
      const notes: string[] = [];
      if (!out.numbered_refs) notes.push(tr('交叉引用未编号（请先编译）', 'cross-references are not numbered (compile first)'));
      if (out.figures_missing.length)
        notes.push(tr(`${out.figures_missing.length} 张图片未能转换`, `${out.figures_missing.length} image(s) could not be converted`));
      const abs = `${this.project.root}/${out.path}`;
      this.flash(
        tr(
          `已导出 ${out.path}${notes.length ? `；${notes.join('，')}` : ''}`,
          `Exported ${out.path}${notes.length ? `; ${notes.join(', ')}` : ''}`,
        ),
        {
        label: tr('在访达中显示', 'Show in Folder'),
        run: () => void revealInFolder(abs),
      });
    } catch (e) {
      this.flash(String(e));
    } finally {
      this.exporting = false;
    }
  }

  /** 在当前文本标签页的光标处插入。没有打开的文本文件时打开主文件。 */
  async insertAtCursor(text: string) {
    let path = this.active;
    if (!path || path.startsWith('yanmo://')) {
      path = this.tabs.find((t) => !t.startsWith('yanmo://')) ?? this.project?.main ?? null;
      if (!path) return;
      await this.openFile(path);
    }
    this.insertRequest = { path, text, n: (this.insertRequest?.n ?? 0) + 1 };
  }

  /** 通过 DOI / arXiv 导入文献，成功后可一键插入 \cite。 */
  async importBib(id: string): Promise<boolean> {
    if (!id.trim() || this.importingBib) return false;
    this.importingBib = true;
    try {
      const fetched = await execute<{ key: string; title: string | null; bibtex: string }>('bib.fetch', { id });
      const added = await execute<{ key: string; duplicate: boolean; file: string; needs_bibliography?: boolean }>('bib.add', {
        bibtex: fetched.bibtex,
      });
      await this.refresh();
      const key = added.key;
      const msg = added.duplicate
        ? tr(`文献库中已有这篇（${key}）`, `Already in the bibliography (${key})`)
        : tr(
            `已导入 ${key} → ${added.file}${added.needs_bibliography ? '；请在正文中用 \\bibliography 引入该文件' : ''}`,
            `Imported ${key} → ${added.file}${added.needs_bibliography ? '; add this file to the paper with \\bibliography' : ''}`,
          );
      this.flash(msg, { label: tr(`插入 \\cite{${key}}`, `Insert \\cite{${key}}`), run: () => void this.insertAtCursor(`\\cite{${key}}`) });
      return true;
    } catch (e) {
      this.flash(String(e));
      return false;
    } finally {
      this.importingBib = false;
    }
  }

  /** 导入图片（拖入的文件路径或粘贴的数据），并在光标处插入 figure 环境。 */
  async importImage(input: { source: string } | { base64: string; name: string }) {
    try {
      const r = await execute<{ path: string; include: string; label: string }>('asset.import', input);
      const snippet = [
        '\\begin{figure}[htbp]',
        '  \\centering',
        `  \\includegraphics[width=0.8\\linewidth]{${r.include}}`,
        '  \\caption{}',
        `  \\label{${r.label}}`,
        '\\end{figure}',
        '',
      ].join('\n');
      await this.insertAtCursor(snippet);
      void execute<TreeNode[]>('project.tree').then((t) => (this.tree = t));
      this.flash(tr(`已把图片放入 ${r.path}，并插入 figure 环境`, `Saved the image to ${r.path} and inserted a figure environment`));
    } catch (e) {
      this.flash(String(e));
    }
  }

  /** 替换 [from, to) 的文本。原文已被改动时会在文档中查找原文；找不到或不唯一则放弃并返回 false。 */
  async replaceRange(path: string, from: number, to: number, expected: string, text: string): Promise<boolean> {
    if (this.active !== path) await this.openFile(path);
    return new Promise((resolve) => {
      this.replaceRequest = { path, from, to, expected, text, n: (this.replaceRequest?.n ?? 0) + 1, resolve };
    });
  }
}

export const app = new AppState();
