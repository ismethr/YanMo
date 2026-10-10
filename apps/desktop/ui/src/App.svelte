<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from './lib/store.svelte';
  import { inTauri } from './lib/backend';
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
  onMount(async () => {
    if (inTauri || !new URLSearchParams(location.search).has('demo')) return;
    await app.openFolder();
    await app.openFile('sections/intro.tex', 3);
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
        <BottomPanel />
      </main>
      {#if app.aiOpen}<AiPanel />{/if}
    </div>
  {:else}
    <Welcome />
  {/if}
  <StatusBar />
  {#if app.palette}<CommandPalette />{/if}
  {#if app.notice}<div class="toast" role="status">{app.notice}</div>{/if}
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
  .toast {
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
