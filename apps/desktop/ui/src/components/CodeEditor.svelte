<script lang="ts">
  // 单个 EditorView，按文件切换 EditorState，保留各文件的撤销历史与光标。
  import { onMount, untrack } from 'svelte';
  import { EditorView } from '@codemirror/view';
  import { EditorState } from '@codemirror/state';
  import { setDiagnostics } from '@codemirror/lint';
  import { app } from '../lib/store.svelte';
  import { diagnosticsFor, extensionsFor } from '../lib/editor';

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
      const head = u.state.selection.main.head;
      const line = u.state.doc.lineAt(head);
      app.cursor = { line: line.number, col: head - line.from + 1 };
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
