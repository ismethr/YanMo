<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { ai } from '../lib/ai-store.svelte';
  import { tr } from '../lib/i18n.svelte';

  let compileErrors = $derived(app.lastCompile?.diagnostics.filter((d) => d.severity === 'error').length ?? 0);
  // 编译日志筛选：排版提示（overfull / underfull box）默认不显示
  let show = $state({ error: true, warning: true, badbox: false });
  let counts = $derived({
    error: compileErrors,
    warning: app.lastCompile?.diagnostics.filter((d) => d.severity === 'warning').length ?? 0,
    badbox: app.lastCompile?.diagnostics.filter((d) => d.severity === 'badbox').length ?? 0,
  });
  // 错误排在前面，同类按日志顺序
  const rank = { error: 0, warning: 1, badbox: 2 } as const;
  let shown = $derived(
    (app.lastCompile?.diagnostics.filter((d) => show[d.severity]) ?? [])
      .map((d, i) => ({ d, i }))
      .sort((a, b) => rank[a.d.severity] - rank[b.d.severity] || a.i - b.i)
      .map((x) => x.d),
  );
  let label = $derived({ error: tr('错误', 'Error'), warning: tr('警告', 'Warning'), badbox: tr('排版', 'Layout') });

  // 面板高度拖拽
  function startResize(e: PointerEvent) {
    const startY = e.clientY;
    const startH = app.layout.panelHeight;
    const move = (ev: PointerEvent) => {
      app.layout.panelHeight = Math.min(window.innerHeight * 0.7, Math.max(90, startH - (ev.clientY - startY)));
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      app.saveLayout();
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }
</script>

<section aria-label={tr('面板', 'Panel')} style:height="{app.layout.panelHeight}px">
  <div class="resizer" role="separator" aria-orientation="horizontal" aria-label={tr('调整面板高度', 'Resize panel')} onpointerdown={startResize}></div>
  <div class="head" role="tablist">
    <button role="tab" aria-selected={app.bottom === 'problems'} class:on={app.bottom === 'problems'} onclick={() => (app.bottom = 'problems')}>
      {tr('问题', 'Problems')} <span class="chip">{app.check?.issues.length ?? 0}</span>
    </button>
    <button role="tab" aria-selected={app.bottom === 'log'} class:on={app.bottom === 'log'} onclick={() => (app.bottom = 'log')}>
      {tr('编译日志', 'Build Log')} {#if compileErrors}<span class="chip c-err">{compileErrors}</span>{/if}
    </button>
    <span class="hint muted">
      {#if app.bottom === 'problems'}{tr('与', 'same as')} <span class="mono">ym check</span>{tr(' 同源', '')}{:else if app.lastCompile}{app.lastCompile.engine} · {(app.lastCompile.duration_ms / 1000).toFixed(1)}s{/if}
    </span>
    <button class="close" aria-label={tr('收起面板（⌘J）', 'Hide panel (⌘J)')} title={tr('收起面板（⌘J）', 'Hide panel (⌘J)')} onclick={() => app.togglePanel()}>×</button>
  </div>
  <div class="body">
    {#if app.bottom === 'problems'}
      {#each app.check?.issues ?? [] as p, i (i)}
        <button class="prob" onclick={() => p.file && app.openFile(p.file, p.line ?? 1)}>
          <span class={p.severity === 'error' ? 'c-err' : 'c-warn'} aria-label={p.severity === 'error' ? label.error : label.warning}>{p.severity === 'error' ? '✕' : '!'}</span>
          <span class="msg">{p.message}</span>
          <span class="mono code">{p.code}</span>
          <span class="loc">{p.file ?? ''}{p.line ? `:${p.line}` : ''}</span>
        </button>
      {:else}
        <p class="c-ok pad">{app.check ? tr('未发现问题', 'No problems found') : tr('尚未检查（项目缺少主文件？）', 'Not checked yet (is the main file missing?)')}</p>
      {/each}
    {:else if app.compiling && !app.lastCompile}
      <p class="muted pad">{tr('编译中…', 'Compiling…')}</p>
    {:else if app.lastCompile}
      <div class="logbar">
        {#each ['error', 'warning', 'badbox'] as const as k (k)}
          <button class="chip" class:on={show[k]} aria-pressed={show[k]} disabled={!counts[k]} onclick={() => (show[k] = !show[k])}>{label[k]} {counts[k]}</button>
        {/each}
        <span class="grow"></span>
        {#if app.lastCompile.log}<button class="lnk" onclick={() => void app.openLog()}>{tr('完整日志', 'Full log')}</button>{/if}
        {#if app.lastCompile.pdf}<button class="lnk" onclick={() => void app.revealPdf()}>{tr('在访达中显示 PDF', 'Show PDF in folder')}</button>{/if}
      </div>
      {#if !shown.length}
        <p class="pad {compileErrors ? 'muted' : 'c-ok'}">
          {app.lastCompile.success ? tr('编译成功，没有需要显示的提示', 'Compiled; nothing to show') : tr('没有符合筛选条件的提示', 'Nothing matches the filter')}
        </p>
      {/if}
      {#each shown as d, i (i)}
        <button class="prob" onclick={() => d.file && app.openFile(d.file, d.line ?? 1)}>
          <span class={d.severity === 'error' ? 'c-err' : 'c-warn'} aria-label={label[d.severity]}>{d.severity === 'error' ? '✕' : '!'}</span>
          <span class="msg">{d.message}</span>
          <span class="loc">{d.file ?? ''}{d.line ? `:${d.line}` : ''}</span>
        </button>
      {/each}
      {#if compileErrors}
        <button class="explain" onclick={() => { app.aiOpen = true; void ai.explainError(); }}>{tr('让 AI 解释这些错误', 'Ask AI to explain these errors')}</button>
      {/if}
      <details>
        <summary class="muted">{tr('原始日志（末尾 40 行）', 'Raw log (last 40 lines)')}</summary>
        <pre class="mono">{app.lastCompile.log_tail}</pre>
      </details>
    {:else}
      <p class="muted pad">
        {tr('还没有编译。按 ⌘B 编译；默认保存后自动编译。', 'Not compiled yet. Press ⌘B to compile; by default the paper also compiles on save.')}
      </p>
    {/if}
  </div>
</section>

<style>
  .logbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px 4px;
  }
  .logbar .grow {
    flex: 1;
  }
  .logbar .chip {
    cursor: pointer;
    padding: 2px 9px;
  }
  .logbar .chip.on {
    background: var(--active);
    color: var(--fg);
    border-color: var(--fg3);
  }
  .logbar .chip:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .lnk {
    font-size: 12px;
    color: var(--accent);
    padding: 0 4px;
  }
  .lnk:hover {
    text-decoration: underline;
  }
  section {
    flex: 0 0 auto;
    position: relative;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--line);
    background: var(--bg);
    min-height: 0;
  }
  .head {
    display: flex;
    gap: 18px;
    padding: 0 14px;
    align-items: center;
  }
  .head button {
    font-size: 12px;
    padding: 8px 2px 6px;
    color: var(--fg2);
    border-bottom: 1px solid transparent;
  }
  .head button.on {
    color: var(--fg);
    border-bottom-color: var(--accent);
  }
  .head > button {
    white-space: nowrap;
  }
  .hint {
    margin-left: auto;
    min-width: 0;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .close {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    text-align: center;
    font-size: 16px;
    color: var(--fg2);
  }
  .close:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .resizer {
    position: absolute;
    top: -3px;
    left: 0;
    right: 0;
    height: 6px;
    cursor: row-resize;
    z-index: 5;
  }
  .resizer:hover,
  .resizer:active {
    background: var(--accent);
    opacity: 0.4;
  }
  .body {
    flex: 1;
    overflow: auto;
    padding-bottom: 6px;
  }
  .prob {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 3px 14px;
  }
  .prob:hover {
    background: var(--hover);
  }
  .prob > span:first-child {
    font-weight: 700;
    width: 12px;
  }
  .msg {
    min-width: 0;
  }
  .code {
    color: var(--fg3);
    font-size: 11px;
  }
  .loc {
    margin-left: auto;
    color: var(--fg2);
    font-size: 12px;
    white-space: nowrap;
  }
  .pad {
    padding: 4px 14px;
    margin: 6px 0;
  }
  details {
    padding: 6px 14px;
  }
  .explain {
    margin: 6px 14px 0;
    font-size: 12px;
    color: var(--tk-cmd);
  }
  .explain:hover {
    text-decoration: underline;
  }
  summary {
    cursor: pointer;
    font-size: 12px;
  }
  pre {
    margin: 6px 0 0;
    font-size: 11.5px;
    line-height: 1.6;
    white-space: pre-wrap;
    color: var(--fg2);
  }
</style>
