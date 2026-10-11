<script lang="ts">
  // 单个 EditorView，按文件切换 EditorState，保留各文件的撤销历史与光标。
  import { onMount, untrack } from 'svelte';
  import { EditorView } from '@codemirror/view';
  import { EditorState } from '@codemirror/state';
  import { setDiagnostics } from '@codemirror/lint';
  import { app } from '../lib/store.svelte';
  import { diagnosticsFor, extensionsFor } from '../lib/editor';
  import { tr } from '../lib/i18n.svelte';

  let { path }: { path: string } = $props();

  let host: HTMLDivElement;
  let view: EditorView | undefined = $state();
  let current: string | null = null;
  let lastReveal = 0;
  const states = new Map<string, EditorState>();

  // 粘贴图片：导入到项目图片目录并插入 figure 环境
  const pasteImages = EditorView.domEventHandlers({
    paste(event) {
      const file = [...(event.clipboardData?.files ?? [])].find((f) => /^image\/(png|jpe?g)$/.test(f.type));
      if (!file) return false;
      event.preventDefault();
      const reader = new FileReader();
      reader.onload = () => {
        const base64 = String(reader.result).split(',')[1] ?? '';
        const ext = file.type === 'image/png' ? 'png' : 'jpg';
        const stamp = new Date().toISOString().slice(0, 19).replace(/[-:T]/g, '');
        void app.importImage({ base64, name: file.name && file.name !== `image.${ext}` ? file.name : `pasted-${stamp}.${ext}` });
      };
      reader.readAsDataURL(file);
      return true;
    },
  });

  const listener = EditorView.updateListener.of((u) => {
    if (!current) return;
    if (u.docChanged) app.edit(current, u.state.doc.toString());
    if (u.docChanged || u.selectionSet) {
      const sel = u.state.selection.main;
      const line = u.state.doc.lineAt(sel.head);
      app.cursor = { line: line.number, col: sel.head - line.from + 1 };
      app.selection = sel.empty
        ? null
        : { path: current, from: sel.from, to: sel.to, text: u.state.sliceDoc(sel.from, sel.to) };
    }
  });

  onMount(() => {
    view = new EditorView({ parent: host });
    return () => view?.destroy();
  });

  // 切换文件
  $effect(() => {
    const p = path;
    const loaded = app.docs[p] !== undefined;
    if (!view || !loaded || current === p) return;
    if (current) states.set(current, view.state);
    const text = untrack(() => app.docs[p].text);
    let st = states.get(p);
    if (!st || st.doc.toString() !== text) {
      st = EditorState.create({ doc: text, extensions: extensionsFor(p, [listener, pasteImages]) });
    }
    view.setState(st);
    current = p;
  });

  // 问题波浪线：检查结果更新或切换文件后重新标注
  $effect(() => {
    const issues = app.issuesFor(path);
    if (!view || current !== path) return;
    view.dispatch(setDiagnostics(view.state, diagnosticsFor(view.state, issues)));
  });

  // 在光标处插入（\cite、figure 环境等）。多行片段放在独立的行上，插入后光标停在 \caption{} 里
  let lastInsert = 0;
  $effect(() => {
    const r = app.insertRequest;
    if (!view || !r || r.n === lastInsert || r.path !== path || current !== path) return;
    lastInsert = r.n;
    const state = view.state;
    const head = state.selection.main.head;
    let text = r.text;
    let at = head;
    if (text.includes('\n')) {
      const line = state.doc.lineAt(head);
      at = line.to;
      if (line.text.trim()) text = `\n${text}`;
    }
    const caption = text.indexOf('\\caption{');
    const cursor = caption >= 0 ? at + caption + '\\caption{'.length : at + text.length;
    view.dispatch({
      changes: { from: at, insert: text },
      selection: { anchor: cursor },
      effects: EditorView.scrollIntoView(cursor, { y: 'center' }),
    });
    view.focus();
  });

  // 文件在磁盘上被改动（移动章节等）：只替换不同的中间部分，光标与撤销历史得以保留
  let lastSync = 0;
  $effect(() => {
    const r = app.syncRequest;
    if (!view || !r || r.n === lastSync) return;
    lastSync = r.n;
    if (r.path !== current) {
      states.delete(r.path);
      return;
    }
    const old = view.state.doc.toString();
    if (old === r.text) return;
    let a = 0;
    while (a < old.length && a < r.text.length && old[a] === r.text[a]) a++;
    let b = 0;
    while (b < old.length - a && b < r.text.length - a && old[old.length - 1 - b] === r.text[r.text.length - 1 - b]) b++;
    view.dispatch({ changes: { from: a, to: old.length - b, insert: r.text.slice(a, r.text.length - b) } });
  });

  // 替换一段文本（AI 改写被接受时）。先校验原文没有被改动，避免覆盖用户之后的编辑
  let lastReplace = 0;
  $effect(() => {
    const r = app.replaceRequest;
    if (!view || !r || r.n === lastReplace || r.path !== path || current !== path) return;
    lastReplace = r.n;
    const doc = view.state.doc.toString();
    let from = r.from;
    if (doc.slice(r.from, r.to) !== r.expected) {
      const at = doc.indexOf(r.expected);
      if (at < 0 || doc.indexOf(r.expected, at + 1) >= 0) {
        app.flash(tr('原文已被修改，无法安全替换；请重新选中后再试', 'The text has changed and cannot be replaced safely. Select it again and retry'));
        r.resolve(false);
        return;
      }
      from = at;
    }
    const to = from + r.expected.length;
    view.dispatch({
      changes: { from, to, insert: r.text },
      selection: { anchor: from, head: from + r.text.length },
      effects: EditorView.scrollIntoView(from, { y: 'center' }),
      userEvent: 'input.ai',
    });
    view.focus();
    r.resolve(true);
  });

  // 跳转到行（来自大纲、问题面板、文献等）
  $effect(() => {
    const r = app.reveal;
    if (!view || !r || r.n === lastReveal || r.path !== path || current !== path) return;
    lastReveal = r.n;
    const line = view.state.doc.line(Math.min(Math.max(1, r.line), view.state.doc.lines));
    view.dispatch({
      selection: { anchor: line.from },
      effects: EditorView.scrollIntoView(line.from, { y: 'center' }),
    });
    view.focus();
  });
</script>

<div class="host" bind:this={host}></div>

<style>
  .host {
    height: 100%;
    overflow: hidden;
  }
</style>
