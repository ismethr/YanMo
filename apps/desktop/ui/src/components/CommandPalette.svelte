<script lang="ts">
  import { onMount } from 'svelte';
  import { app, PDF_TAB } from '../lib/store.svelte';
  import { listCommands } from '../lib/backend';
  import type { CommandMeta } from '../lib/types';
  import { ai, rewriteLabels } from '../lib/ai-store.svelte';
  import { i18n, tr } from '../lib/i18n.svelte';
  import { updater } from '../lib/updater.svelte';
  import type { RewriteAction } from '../lib/ai';

  interface Item {
    id: string;
    title: string;
    key?: string;
    run: () => void;
  }

  // 界面命令；后端命令注册表中的命令一并列出（只读展示，供 Agent / CLI 对照 ID）
  const uiItems: Item[] = $derived([
    { id: 'project.open', title: tr('打开文件夹…', 'Open Folder…'), key: '⌘O', run: () => app.openFolder() },
    { id: 'file.save', title: tr('保存当前文件', 'Save File'), key: '⌘S', run: () => app.saveActive() },
    { id: 'compile.run', title: tr('编译论文', 'Compile Paper'), key: '⌘B', run: () => app.compile() },
    { id: 'synctex.forward', title: tr('在 PDF 中定位当前行', 'Show Current Line in PDF'), key: '⌥⌘J', run: () => app.forwardSync() },
    { id: 'export.docx', title: tr('导出为 Word（.docx）', 'Export to Word (.docx)'), run: () => app.exportDocx() },
    {
      id: 'compile.auto',
      title: tr('切换：保存后自动编译', 'Toggle: Compile on Save'),
      run: () => {
        app.autoCompile = !app.autoCompile;
        app.flash(
          app.autoCompile ? tr('已开启保存后自动编译', 'Compile on save is on') : tr('已关闭保存后自动编译', 'Compile on save is off'),
        );
      },
    },
    { id: 'check.run', title: tr('重新检查项目', 'Check Project Again'), run: () => { app.showPanel('problems'); void app.refresh(); } },
    { id: 'view.outline', title: tr('显示大纲', 'Show Outline'), run: () => (app.activity = 'outline') },
    { id: 'view.files', title: tr('显示文件', 'Show Files'), run: () => (app.activity = 'files') },
    { id: 'view.assets', title: tr('显示图表资产', 'Show Figures & Tables'), run: () => (app.activity = 'assets') },
    { id: 'view.bib', title: tr('显示文献库', 'Show Bibliography'), run: () => (app.activity = 'bib') },
    { id: 'view.open_pdf', title: tr('打开 PDF 预览', 'Open PDF Preview'), run: () => app.openFile(PDF_TAB) },
    { id: 'view.toggle_panel', title: tr('切换底部面板', 'Toggle Bottom Panel'), key: '⌘J', run: () => app.togglePanel() },
    ...(Object.entries(rewriteLabels()) as [RewriteAction, string][]).map(([a, label]) => ({
      id: `ai.${a}`,
      title: tr(`AI：${label}选中的文字`, `AI: ${label} (selection)`),
      run: () => void ai.rewrite(a),
    })),
    { id: 'ai.explain_error', title: tr('AI：解释编译错误', 'AI: Explain Build Errors'), run: () => { app.aiOpen = true; void ai.explainError(); } },
    { id: 'ai.settings', title: tr('AI 设置', 'AI Settings'), run: () => { app.aiOpen = true; ai.showSettings = true; } },
    {
      id: 'mcp.copy',
      title: tr('接入 Claude Code 等 Agent（复制 MCP 命令）', 'Connect Claude Code or Other Agents (Copy MCP Command)'),
      run: () => void app.copyMcpCommand(),
    },
    { id: 'view.toggle_ai', title: tr('切换 AI 面板', 'Toggle AI Panel'), key: '⌘I', run: () => (app.aiOpen = !app.aiOpen) },
    {
      id: 'outline.extract',
      title: tr('把当前章节拆成独立文件', 'Move Current Section to Its Own File'),
      run: () => (app.currentSectionId ? void app.extractSection(app.currentSectionId) : app.flash(tr('光标不在任何章节中', 'The cursor is not in a section'))),
    },
    { id: 'app.update', title: tr('检查更新', 'Check for Updates'), run: () => void updater.check() },
    {
      id: 'app.set_lang',
      title: i18n.lang === 'zh' ? 'Switch to English（切换为英文界面）' : '切换为中文界面 (Switch to Chinese)',
      run: () => void app.switchLang(i18n.lang === 'zh' ? 'en' : 'zh'),
    },
  ]);

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
      .map((c) => ({
        id: c.id,
        title: tr(`${c.title}（后端命令）`, `${c.title_en} (backend command)`),
        run: () => app.flash(tr(`可用 ym exec ${c.id} 调用`, `Run it with ym exec ${c.id}`)),
      }));
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
  <div class="dialog" role="dialog" aria-label={tr('命令面板', 'Command palette')} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={onKey}>
    <label class="sr-only" for="pq">{tr('搜索命令', 'Search commands')}</label>
    <input id="pq" bind:this={input} bind:value={q} oninput={() => (sel = 0)} placeholder={tr('> 输入命令，如 open、大纲、check', '> Type a command, e.g. open, outline, check')} />
    <div class="list" role="listbox">
      {#each items as it, i (it.id)}
        <button role="option" aria-selected={i === sel} class:sel={i === sel} onclick={() => run(it)} onmouseenter={() => (sel = i)}>
          <span>{it.title}</span>
          <span class="mono id">{it.id}</span>
          {#if it.key}<span class="key">{it.key}</span>{/if}
        </button>
      {:else}
        <p class="muted none">{tr('没有匹配的命令', 'No matching commands')}</p>
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
