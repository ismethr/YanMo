// AI 面板的状态与操作：对话、选区改写（diff 确认后才写入）、解释编译错误。

import { app } from './store.svelte';
import { execute } from './backend';
import { tr } from './i18n.svelte';
import { cancelAi, getAiSettings, runAi, type AiContext, type AiSettingsInfo, type AiTask, type RewriteAction } from './ai';

export function rewriteLabels(): Record<RewriteAction, string> {
  return {
    polish: tr('润色', 'Polish'),
    academic: tr('学术化', 'Academic'),
    translate_to_english: tr('译为英文', 'To English'),
    translate_to_chinese: tr('译为中文', 'To Chinese'),
    shorten: tr('缩写', 'Shorten'),
    expand: tr('扩写', 'Expand'),
  };
}

/** 单次改写的选区上限（字符），过长时提示分段 */
const MAX_SELECTION = 12_000;

export type AiMessage =
  | { id: number; kind: 'user'; text: string }
  | { id: number; kind: 'assistant'; text: string; streaming: boolean; model?: string; error?: string }
  | { id: number; kind: 'notice'; text: string }
  | {
      id: number;
      kind: 'proposal';
      label: string;
      path: string;
      from: number;
      to: number;
      original: string;
      raw: string;
      rewrite: string | null;
      status: 'streaming' | 'ready' | 'accepted' | 'rejected' | 'stale' | 'error';
      error?: string;
      model?: string;
    };

let nextId = 1;

class AiPanelState {
  messages = $state<AiMessage[]>([]);
  running = $state<string | null>(null);
  info = $state<AiSettingsInfo | null>(null);
  showSettings = $state(false);

  get configured(): boolean {
    const i = this.info;
    if (!i) return false;
    return i.settings.provider === 'anthropic'
      ? i.has_key.anthropic
      : !!i.settings.openai_base_url && !!i.settings.openai_model;
  }

  get modelName(): string {
    const s = this.info?.settings;
    if (!s) return '';
    return s.provider === 'anthropic' ? s.anthropic_model : s.openai_model || tr('未选择模型', 'No model selected');
  }

  async loadSettings() {
    this.info = await getAiSettings();
  }

  /** 发送给模型的上下文：当前文件、当前章节源码、大纲。界面上以标签展示。 */
  context(): AiContext {
    const rows = app.outlineRows;
    const outline = rows
      .slice(0, 200)
      .map((r) => `${'  '.repeat(r.depth)}${r.number ? `${r.number} ` : ''}${r.title}`)
      .join('\n');
    const path = app.active && !app.active.startsWith('yanmo://') ? app.active : null;
    let section_title: string | null = null;
    let section_text: string | null = null;
    const cur = rows.find((r) => r.id === app.currentSectionId);
    const doc = path ? app.docs[path] : undefined;
    if (cur && doc) {
      const lines = doc.text.split('\n');
      const next = rows.slice(rows.indexOf(cur) + 1).find((r) => r.file === cur.file && r.depth <= cur.depth);
      const end = next ? next.line - 1 : lines.length;
      section_title = cur.title;
      section_text = lines.slice(cur.line - 1, end).join('\n').slice(0, 30_000);
    }
    return { file: path, section_title, section_text, outline: outline || null };
  }

  /** 上下文标签（展示给用户，说明会发送哪些内容） */
  get contextChips(): string[] {
    const chips: string[] = [];
    const sel = app.selection;
    if (sel?.text.trim()) chips.push(tr(`选区 ${sel.text.length} 字符`, `Selection: ${sel.text.length} chars`));
    const cur = app.outlineRows.find((r) => r.id === app.currentSectionId);
    if (cur) chips.push(`${tr('当前章节：', 'Section: ')}${cur.number ? `${cur.number} ` : ''}${cur.title}`);
    if (app.outlineRows.length) chips.push(tr('大纲', 'Outline'));
    return chips;
  }

  #start(task: AiTask, onText: (t: string) => void, onNotice: (n: string) => void) {
    const run = runAi(task, this.context(), onText, onNotice);
    this.running = run.id;
    return run;
  }

  stop() {
    if (this.running) cancelAi(this.running);
  }

  async send(text: string) {
    const t = text.trim();
    if (!t || this.running) return;
    this.messages.push({ id: nextId++, kind: 'user', text: t });
    const history = this.messages
      .filter((m): m is Extract<AiMessage, { kind: 'user' | 'assistant' }> => m.kind === 'user' || (m.kind === 'assistant' && !m.error && !m.streaming))
      .map((m) => ({ role: m.kind as 'user' | 'assistant', content: m.text }));
    await this.#stream({ kind: 'chat', history });
  }

  async explainError() {
    const errors = app.lastCompile?.diagnostics.filter((d) => d.severity === 'error') ?? [];
    if (!errors.length) {
      app.flash(tr('最近一次编译没有错误', 'The last build had no errors'));
      return;
    }
    const diagnostics = errors
      .slice(0, 10)
      .map((d) => `${d.file ?? '?'}${d.line ? `:${d.line}` : ''}: ${d.message}`)
      .join('\n');
    const first = errors.find((d) => d.file && d.line);
    let source = '';
    if (first?.file && first.line) {
      let text: string | undefined = app.docs[first.file]?.text;
      if (text === undefined) {
        text = (await execute<{ text: string }>('file.read', { path: first.file }).catch(() => null))?.text;
      }
      if (text !== undefined) {
        const lines = text.split('\n');
        const from = Math.max(0, first.line - 8);
        source = lines
          .slice(from, first.line + 6)
          .map((l, i) => `${from + i + 1}: ${l}`)
          .join('\n');
      }
    }
    this.messages.push({ id: nextId++, kind: 'user', text: tr('解释最近一次编译的错误', 'Explain the errors from the last build') });
    await this.#stream({ kind: 'explain_error', diagnostics, source });
  }

  async #stream(task: AiTask) {
    const msg: AiMessage = { id: nextId++, kind: 'assistant', text: '', streaming: true };
    this.messages.push(msg);
    const m = this.messages[this.messages.length - 1] as Extract<AiMessage, { kind: 'assistant' }>;
    const run = this.#start(
      task,
      (t) => (m.text += t),
      (n) => this.messages.splice(this.messages.indexOf(m), 0, { id: nextId++, kind: 'notice', text: n }),
    );
    try {
      const r = await run.done;
      m.model = r.outcome.model;
    } catch (e) {
      m.error = String(e).replace(/^Error:\s*/, '');
    } finally {
      m.streaming = false;
      this.running = null;
    }
  }

  /** 改写当前选区：流式生成 → diff 卡片 → 用户接受后替换（可撤销）。 */
  async rewrite(action: RewriteAction) {
    const sel = app.selection;
    if (this.running) return;
    if (!sel || !sel.text.trim()) {
      app.flash(tr('先在编辑器中选中要处理的文字', 'Select some text in the editor first'));
      return;
    }
    if (sel.text.length > MAX_SELECTION) {
      app.flash(
        tr(
          `选区过长（${sel.text.length} 字符），请分段处理（上限 ${MAX_SELECTION}）`,
          `Selection too long (${sel.text.length} chars). Work in smaller parts (limit ${MAX_SELECTION})`,
        ),
      );
      return;
    }
    app.aiOpen = true;
    this.messages.push({
      id: nextId++,
      kind: 'proposal',
      label: rewriteLabels()[action],
      path: sel.path,
      from: sel.from,
      to: sel.to,
      original: sel.text,
      raw: '',
      rewrite: null,
      status: 'streaming',
    });
    const p = this.messages[this.messages.length - 1] as Extract<AiMessage, { kind: 'proposal' }>;
    const run = this.#start(
      { kind: 'rewrite', action, selection: sel.text },
      (t) => (p.raw += t),
      (n) => this.messages.splice(this.messages.indexOf(p), 0, { id: nextId++, kind: 'notice', text: n }),
    );
    try {
      const r = await run.done;
      // 保留原选区首尾的空白与换行（选中整行时常带着行尾换行），只替换中间的正文
      const lead = p.original.match(/^\s*/)?.[0] ?? '';
      const trail = p.original.match(/\s*$/)?.[0] ?? '';
      const body = (r.rewrite ?? r.outcome.text).trim();
      p.rewrite = body ? lead + body + trail : null;
      if (!body) throw new Error(tr('模型没有返回改写内容', 'The model returned no rewrite'));
      p.model = r.outcome.model;
      p.status = 'ready';
    } catch (e) {
      p.error = String(e).replace(/^Error:\s*/, '');
      p.status = 'error';
    } finally {
      this.running = null;
    }
  }

  async accept(p: Extract<AiMessage, { kind: 'proposal' }>) {
    if (p.status !== 'ready' || p.rewrite === null) return;
    const ok = await app.replaceRange(p.path, p.from, p.to, p.original, p.rewrite);
    p.status = ok ? 'accepted' : 'stale';
  }

  reject(p: Extract<AiMessage, { kind: 'proposal' }>) {
    p.status = 'rejected';
  }

  clear() {
    if (!this.running) this.messages = [];
  }
}

export const ai = new AiPanelState();
