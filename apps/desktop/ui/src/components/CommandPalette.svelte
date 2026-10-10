<script lang="ts">
  import { onMount } from 'svelte';
  import { app, PDF_TAB } from '../lib/store.svelte';
  import { listCommands } from '../lib/backend';
  import type { CommandMeta } from '../lib/types';
  import { ai, REWRITE_LABELS } from '../lib/ai-store.svelte';
  import type { RewriteAction } from '../lib/ai';

  interface Item {
    id: string;
    title: string;
    key?: string;
    run: () => void;
  }

  // 界面命令；后端命令注册表中的命令一并列出（只读展示，供 Agent / CLI 对照 ID）
  const uiItems: Item[] = [
    { id: 'project.open', title: '打开文件夹…', key: '⌘O', run: () => app.openFolder() },
    { id: 'file.save', title: '保存当前文件', key: '⌘S', run: () => app.saveActive() },
    { id: 'compile.run', title: '编译论文', key: '⌘B', run: () => app.compile() },
    { id: 'synctex.forward', title: '在 PDF 中定位当前行', key: '⌥⌘J', run: () => app.forwardSync() },
    { id: 'export.docx', title: '导出为 Word（.docx）', run: () => app.exportDocx() },
    { id: 'compile.auto', title: '切换：保存后自动编译', run: () => { app.autoCompile = !app.autoCompile; app.flash(app.autoCompile ? '已开启保存后自动编译' : '已关闭保存后自动编译'); } },
    { id: 'check.run', title: '重新检查项目', run: () => { app.showPanel('problems'); void app.refresh(); } },
    { id: 'view.outline', title: '显示大纲', run: () => (app.activity = 'outline') },
    { id: 'view.files', title: '显示文件', run: () => (app.activity = 'files') },
    { id: 'view.assets', title: '显示图表资产', run: () => (app.activity = 'assets') },
    { id: 'view.bib', title: '显示文献库', run: () => (app.activity = 'bib') },
    { id: 'view.open_pdf', title: '打开 PDF 预览', run: () => app.openFile(PDF_TAB) },
    { id: 'view.toggle_panel', title: '切换底部面板', key: '⌘J', run: () => app.togglePanel() },
    ...(Object.entries(REWRITE_LABELS) as [RewriteAction, string][]).map(([a, label]) => ({
      id: `ai.${a}`,
      title: `AI：${label}选中的文字`,
      run: () => void ai.rewrite(a),
    })),
    { id: 'ai.explain_error', title: 'AI：解释编译错误', run: () => { app.aiOpen = true; void ai.explainError(); } },
    { id: 'ai.settings', title: 'AI 设置', run: () => { app.aiOpen = true; ai.showSettings = true; } },
    { id: 'view.toggle_ai', title: '切换 AI 面板', key: '⌘I', run: () => (app.aiOpen = !app.aiOpen) },
  ];

  let q = $state('');
  let sel = $state(0);
  let backend = $state<CommandMeta[]>([]);
  let input: HTMLInputElement;

  onMount(() => {
    input.focus();
    listCommands().then((c) => (backend = c)).catch(() => {});
  });

  let items = $derived.by(() => {
    const ids = new Set(uiItems.map((i) => i.id));
    const extra: Item[] = backend
      .filter((c) => !ids.has(c.id))
      .map((c) => ({ id: c.id, title: `${c.title}（后端命令）`, run: () => app.flash(`可用 ym exec ${c.id} 调用`) }));
    const s = q.trim().toLowerCase();
    return [...uiItems, ...extra].filter((i) => !s || i.id.includes(s) || i.title.toLowerCase().includes(s));
  });

  function run(i: Item) {
    app.palette = false;
    i.run();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') app.palette = false;
    else if (e.key === 'ArrowDown') {
      e.preventDefault();
      sel = Math.min(sel + 1, items.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      sel = Math.max(sel - 1, 0);
    } else if (e.key === 'Enter' && items[sel]) run(items[sel]);
  }
</script>

<div class="backdrop" role="presentation" onclick={() => (app.palette = false)}>
  <div class="dialog" role="dialog" aria-label="命令面板" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={onKey}>
    <label class="sr-only" for="pq">搜索命令</label>
    <input id="pq" bind:this={input} bind:value={q} oninput={() => (sel = 0)} placeholder="> 输入命令，如 open、大纲、check" />
    <div class="list" role="listbox">
      {#each items as it, i (it.id)}
        <button role="option" aria-selected={i === sel} class:sel={i === sel} onclick={() => run(it)} onmouseenter={() => (sel = i)}>
          <span>{it.title}</span>
          <span class="mono id">{it.id}</span>
          {#if it.key}<span class="key">{it.key}</span>{/if}
        </button>
      {:else}
        <p class="muted none">没有匹配的命令</p>
      {/each}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.18);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 52px;
    z-index: 10;
  }
  .dialog {
    width: min(600px, 92%);
    background: var(--bg2);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.18);
    overflow: hidden;
  }
  input {
    width: calc(100% - 16px);
    margin: 8px;
    height: 30px;
    border-radius: 5px;
    border: 1px solid var(--accent);
    background: var(--bg);
    padding: 0 10px;
    outline: none;
  }
  .list {
    max-height: 360px;
    overflow: auto;
    padding-bottom: 4px;
  }
  .list button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 14px;
  }
  .list button.sel {
    background: var(--sel);
  }
  .id {
    font-size: 11px;
    color: var(--fg3);
  }
  .key {
    margin-left: auto;
    font-size: 12px;
    color: var(--fg2);
  }
  .none {
    padding: 8px 14px;
    margin: 0;
  }
</style>
