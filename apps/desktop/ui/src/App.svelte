<script lang="ts">
  import { onMount } from 'svelte';
  import { app, PDF_TAB } from './lib/store.svelte';
  import { inTauri, launchFolder } from './lib/backend';
  import { syncBackendLang, tr } from './lib/i18n.svelte';
  import { updater } from './lib/updater.svelte';
  import TitleBar from './components/TitleBar.svelte';
  import ActivityBar from './components/ActivityBar.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import EditorArea from './components/EditorArea.svelte';
  import BottomPanel from './components/BottomPanel.svelte';
  import AiPanel from './components/AiPanel.svelte';
  import StatusBar from './components/StatusBar.svelte';
  import Welcome from './components/Welcome.svelte';
  import CommandPalette from './components/CommandPalette.svelte';

  // 浏览器预览的演示模式：?demo 自动打开示例论文（用于截图与官网）
  // 从访达拖入图片：导入并插入 figure 环境
  let dragging = $state(false);
  onMount(() => {
    if (!inTauri) return;
    let unlisten: (() => void) | undefined;
    void import('@tauri-apps/api/webview').then(async ({ getCurrentWebview }) => {
      unlisten = await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === 'enter' || p.type === 'over') dragging = !!app.project;
        else if (p.type === 'leave') dragging = false;
        else if (p.type === 'drop') {
          dragging = false;
          if (!app.project) return;
          const images = p.paths.filter((x) => /\.(png|jpe?g|pdf|eps)$/i.test(x));
          if (!images.length) {
            app.flash(tr('只能拖入 PNG、JPG、PDF、EPS 图片', 'Only PNG, JPG, PDF and EPS images can be dropped'));
            return;
          }
          void (async () => {
            for (const source of images) await app.importImage({ source });
          })();
        }
      });
    });
    return () => unlisten?.();
  });

  onMount(async () => {
    await syncBackendLang();
    // 启动稍后再检查更新，不和打开项目抢资源
    setTimeout(() => void updater.autoCheck(), 5000);
    if (inTauri) {
      const folder = await launchFolder();
      if (folder) await app.openFolder(folder);
      return;
    }
    const q = new URLSearchParams(location.search);
    if (!q.has('demo')) return;
    await app.openFolder();
    await app.openFile('sections/intro.tex', 3);
    // ?demo&pdf=<url>：在预览模式中加载指定 PDF，便于调试查看器
    const pdf = q.get('pdf');
    if (pdf) {
      const data = new Uint8Array(await (await fetch(pdf)).arrayBuffer());
      app.pdf = { data, rev: (app.pdf?.rev ?? 0) + 1 };
      await app.openFile(PDF_TAB);
    }
  });

  function onKey(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    const k = e.key.toLowerCase();
    if (k === 's') {
      e.preventDefault();
      void app.saveActive();
    } else if (k === 'p' && e.shiftKey) {
      e.preventDefault();
      app.palette = !app.palette;
    } else if (k === 'i') {
      e.preventDefault();
      app.aiOpen = !app.aiOpen;
    } else if (k === 'j' && !e.altKey) {
      e.preventDefault();
      app.togglePanel();
    } else if (k === 'b') {
      e.preventDefault();
      void app.compile();
    } else if (e.altKey && (k === 'j' || e.code === 'KeyJ')) {
      e.preventDefault();
      void app.forwardSync();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="shell">
  <TitleBar />
  {#if app.project}
    <div class="middle">
      <ActivityBar />
      <Sidebar />
      <main>
        <EditorArea />
        {#if app.layout.panelOpen}<BottomPanel />{/if}
      </main>
      {#if app.aiOpen}<AiPanel />{/if}
    </div>
  {:else}
    <Welcome />
  {/if}
  <StatusBar />
  {#if app.palette}<CommandPalette />{/if}
  {#if dragging}
    <div class="dropzone" aria-hidden="true">
      <div>
        {tr('松开以导入图片', 'Drop to import images')}<br /><span
          >{tr('放入项目图片目录，并在光标处插入 figure 环境', 'Saved to the project image folder and inserted as a figure at the cursor')}</span
        >
      </div>
    </div>
  {/if}
  {#if app.notice}
    <div class="toast" role="status">
      <span>{app.notice.text}</span>
      {#if app.notice.action}
        <button class="btn" onclick={() => { app.notice?.action?.run(); app.notice = null; }}>{app.notice.action.label}</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  .middle {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .dropzone {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.72);
    border: 2px dashed var(--accent);
    pointer-events: none;
    font-size: 18px;
    font-weight: 500;
    text-align: center;
    line-height: 1.8;
  }
  .dropzone span {
    font-size: 13px;
    font-weight: 400;
    color: var(--fg2);
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    position: absolute;
    right: 16px;
    bottom: 36px;
    max-width: 420px;
    padding: 10px 14px;
    border-radius: 6px;
    background: var(--bg2);
    border: 1px solid var(--line);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.12);
  }
</style>
