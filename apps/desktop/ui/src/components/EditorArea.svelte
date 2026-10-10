<script lang="ts">
  import { app, PDF_TAB, IMAGE_TAB } from '../lib/store.svelte';
  import Thumb from './Thumb.svelte';
  import CodeEditor from './CodeEditor.svelte';
  import PdfViewer from './PdfViewer.svelte';
  import Icon from './Icon.svelte';
  import { tr } from '../lib/i18n.svelte';

  let pdfName = $derived(`${(app.project?.main ?? 'main.tex').split('/').pop()!.replace(/\.tex$/, '')}.pdf`);
  const isText = (p: string | null) => !!p && !p.startsWith('yanmo://');
  const name = (p: string) => (p === PDF_TAB ? pdfName : p.split('/').pop()!);
  let imagePath = $derived(app.active?.startsWith(IMAGE_TAB) ? app.active.slice(IMAGE_TAB.length) : null);
  let crumbs = $derived(isText(app.active) ? app.active!.split('/') : []);
  let hasError = (p: string) => app.issuesFor(p).some((i) => i.severity === 'error');
  // 切到 PDF 时编辑器只隐藏不销毁，保留撤销历史与光标
  let textTab = $state<string | null>(null);
  $effect(() => {
    if (isText(app.active)) textTab = app.active;
    else if (!app.active || !app.tabs.includes(textTab ?? '')) textTab = app.tabs.find(isText) ?? null;
  });
</script>

<section aria-label={tr('编辑器', 'Editor')}>
  <div class="tabs" role="tablist">
    {#each app.tabs as t (t)}
      <div class="tab" class:on={app.active === t} role="presentation">
        <button role="tab" aria-selected={app.active === t} onclick={() => (app.active = t)}>
          {#if t === PDF_TAB}<Icon name="pdf" size={13} />{:else if t.startsWith(IMAGE_TAB)}<Icon name="assets" size={13} />{/if}
          <span class:err={hasError(t)}>{name(t)}</span>
        </button>
        <button class="x" aria-label={tr(`关闭 ${name(t)}`, `Close ${name(t)}`)} onclick={() => app.closeTab(t)}>
          {#if app.isDirty(t)}<span class="dot">●</span>{:else}<Icon name="close" size={12} />{/if}
        </button>
      </div>
    {/each}
    <button class="pdf-open" onclick={() => app.openFile(PDF_TAB)} title={tr('打开 PDF 预览', 'Open PDF preview')}>
      <Icon name="pdf" size={14} />PDF
    </button>
  </div>

  {#if app.active === PDF_TAB && !app.pdf}
    <div class="crumbs">{pdfName}</div>
    <div class="pdf">
      <div class="paper">
        <p class="strong">{tr('还没有 PDF', 'No PDF yet')}</p>
        <p class="muted">
          {tr(
            '点击编译（⌘B）生成 PDF。默认使用内置 Tectonic，首次编译需要下载宏包，可能要几分钟。',
            'Click Compile (⌘B) to build the PDF. The built-in Tectonic engine is used by default; the first build downloads packages and may take a few minutes.',
          )}
        </p>
        <button class="btn btn-pri" onclick={() => app.compile()} disabled={app.compiling}>{app.compiling ? tr('编译中…', 'Compiling…') : tr('编译', 'Compile')}</button>
      </div>
    </div>
  {:else if app.active === PDF_TAB}
    <!-- PDF 查看器在下方常驻渲染 -->
  {:else if imagePath}
    <div class="crumbs">{imagePath.split('/').join(' › ')}</div>
    <div class="image-view">
      {#key imagePath}<Thumb path={imagePath} width={900} height={640} fallback={tr('加载中…', 'Loading…')} />{/key}
    </div>
  {:else if app.active}
    <div class="crumbs">
      {#each crumbs as c, i (i)}{#if i}<span aria-hidden="true">›</span>{/if}<span>{c}</span>{/each}
      {#if app.active.endsWith('.tex')}
        <button class="sync" onclick={() => app.forwardSync()} title={tr('在 PDF 中定位当前行（⌥⌘J）', 'Show current line in PDF (⌥⌘J)')}>
          <Icon name="pdf" size={12} />{tr('在 PDF 中定位', 'Show in PDF')}
        </button>
      {/if}
    </div>
  {:else}
    <div class="blank">
      <p class="muted">{tr('从左侧打开一个文件，或按 ⇧⌘P 打开命令面板', 'Open a file on the left, or press ⇧⌘P for the command palette')}</p>
    </div>
  {/if}
  {#if app.pdf}
    <div class="viewer" class:hidden={app.active !== PDF_TAB}><PdfViewer /></div>
  {/if}
  {#if textTab}
    <div class="editor" class:hidden={!isText(app.active)}><CodeEditor path={textTab} /></div>
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
  .image-view {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--pdfbg);
    padding: 24px;
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
