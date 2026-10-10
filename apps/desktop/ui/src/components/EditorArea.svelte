<script lang="ts">
  import { app, PDF_TAB } from '../lib/store.svelte';
  import CodeEditor from './CodeEditor.svelte';
  import PdfViewer from './PdfViewer.svelte';
  import Icon from './Icon.svelte';

  const name = (p: string) => (p === PDF_TAB ? 'main.pdf' : p.split('/').pop()!);
  let crumbs = $derived(app.active && app.active !== PDF_TAB ? app.active.split('/') : []);
  let hasError = (p: string) => app.issuesFor(p).some((i) => i.severity === 'error');
  // 切到 PDF 时编辑器只隐藏不销毁，保留撤销历史与光标
  let textTab = $state<string | null>(null);
  $effect(() => {
    if (app.active && app.active !== PDF_TAB) textTab = app.active;
    else if (!app.active || !app.tabs.includes(textTab ?? '')) textTab = app.tabs.find((t) => t !== PDF_TAB) ?? null;
  });
</script>

<section aria-label="编辑器">
  <div class="tabs" role="tablist">
    {#each app.tabs as t (t)}
      <div class="tab" class:on={app.active === t} role="presentation">
        <button role="tab" aria-selected={app.active === t} onclick={() => (app.active = t)}>
          {#if t === PDF_TAB}<Icon name="pdf" size={13} />{/if}
          <span class:err={hasError(t)}>{name(t)}</span>
        </button>
        <button class="x" aria-label="关闭 {name(t)}" onclick={() => app.closeTab(t)}>
          {#if app.isDirty(t)}<span class="dot">●</span>{:else}<Icon name="close" size={12} />{/if}
        </button>
      </div>
    {/each}
    <button class="pdf-open" onclick={() => app.openFile(PDF_TAB)} title="打开 PDF 预览">
      <Icon name="pdf" size={14} />PDF
    </button>
  </div>

  {#if app.active === PDF_TAB && !app.pdf}
    <div class="crumbs">main.pdf</div>
    <div class="pdf">
      <div class="paper">
        <p class="strong">还没有 PDF</p>
        <p class="muted">点击编译（⌘B）生成 PDF。默认使用内置 Tectonic，首次编译需要下载宏包，可能要几分钟。</p>
        <button class="btn btn-pri" onclick={() => app.compile()} disabled={app.compiling}>{app.compiling ? '编译中…' : '编译'}</button>
      </div>
    </div>
  {:else if app.active === PDF_TAB}
    <!-- PDF 查看器在下方常驻渲染 -->
  {:else if app.active}
    <div class="crumbs">
      {#each crumbs as c, i (i)}{#if i}<span aria-hidden="true">›</span>{/if}<span>{c}</span>{/each}
      {#if app.active.endsWith('.tex')}
        <button class="sync" onclick={() => app.forwardSync()} title="在 PDF 中定位当前行（⌥⌘J）">
          <Icon name="pdf" size={12} />在 PDF 中定位
        </button>
      {/if}
    </div>
  {:else}
    <div class="blank">
      <p class="muted">从左侧打开一个文件，或按 ⇧⌘P 打开命令面板</p>
    </div>
  {/if}
  {#if app.pdf}
    <div class="viewer" class:hidden={app.active !== PDF_TAB}><PdfViewer /></div>
  {/if}
  {#if textTab}
    <div class="editor" class:hidden={app.active === PDF_TAB}><CodeEditor path={textTab} /></div>
  {/if}
</section>

<style>
  section {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .tabs {
    display: flex;
    background: var(--bg2);
    border-bottom: 1px solid var(--line);
    overflow-x: auto;
    flex: 0 0 auto;
  }
  .tab {
    display: flex;
    align-items: center;
    height: 35px;
    border-right: 1px solid var(--line);
    color: var(--fg2);
  }
  .tab.on {
    background: var(--bg);
    color: var(--fg);
    box-shadow: inset 0 1px 0 var(--accent);
  }
  .tab > button:first-child {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 100%;
    padding: 0 6px 0 14px;
    white-space: nowrap;
  }
  .err {
    text-decoration: underline wavy var(--err);
    text-underline-offset: 4px;
  }
  .x {
    width: 22px;
    height: 22px;
    margin-right: 6px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg3);
  }
  .x:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .dot {
    font-size: 10px;
  }
  .pdf-open {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 0 12px;
    color: var(--fg2);
    font-size: 12px;
  }
  .pdf-open:hover {
    color: var(--fg);
  }
  .crumbs {
    height: 24px;
    flex: 0 0 24px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 14px;
    font-size: 12px;
    color: var(--fg2);
    border-bottom: 1px solid var(--line);
  }
  .editor {
    flex: 1;
    min-height: 0;
  }
  .hidden {
    display: none !important;
  }
  .viewer {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .sync {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 1px 8px;
    border-radius: 4px;
    color: var(--fg2);
  }
  .sync:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .paper .btn {
    margin-top: 8px;
  }
  .pdf {
    flex: 1;
    overflow: auto;
    background: var(--pdfbg);
    padding: 28px;
  }
  .paper {
    max-width: 620px;
    min-height: 420px;
    margin: 0 auto;
    background: #fff;
    color: #222;
    padding: 56px 60px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12), 0 6px 24px rgba(0, 0, 0, 0.1);
    line-height: 1.7;
  }
  .strong {
    font-weight: 600;
    font-size: 15px;
  }
  .blank {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
