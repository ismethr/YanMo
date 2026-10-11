<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import { tr } from '../lib/i18n.svelte';
</script>

<header>
  <div class="brand">
    <img class="logo" src="/icon.svg" alt="" width="22" height="22" />
    <span>{tr('研墨', 'YanMo')}</span>
  </div>
  <button class="center" onclick={() => (app.palette = true)}>
    <Icon name="search" size={14} />
    <span>{app.project ? `${app.project.name} — ` : ''}{tr('搜索命令、文件、label、文献', 'Search commands, files, labels…')}</span>
    <span class="kbd">⇧⌘P</span>
  </button>
  <div class="actions">
    {#if app.compiling}
      <button class="btn btn-pri" onclick={() => void app.cancelCompile()} title={tr('停止编译', 'Stop compiling')}>
        <span class="stop" aria-hidden="true"></span>{tr('停止', 'Stop')} · {app.compileElapsed}s
      </button>
    {:else}
    <button class="btn btn-pri" disabled={!app.project} onclick={() => app.compile()} title={tr('编译（⌘B）', 'Compile (⌘B)')}>
      <Icon name="play" size={12} fill />{tr('编译', 'Compile')}
    </button>
    {/if}
    <button class="btn" disabled={!app.project || app.exporting} onclick={() => app.exportDocx()} title={tr('用 Pandoc 导出为 Word（.docx）', 'Export to Word (.docx) with Pandoc')}>
      <Icon name="word" size={14} />{app.exporting ? tr('导出中…', 'Exporting…') : 'Word'}
    </button>
    <button class="btn" onclick={() => (app.aiOpen = !app.aiOpen)} aria-pressed={app.aiOpen}>
      <Icon name="sparkle" size={14} />AI
    </button>
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    padding: 0 10px;
    background: var(--bg3);
    border-bottom: 1px solid var(--line);
    flex: 0 0 auto;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    width: 300px;
  }
  .center {
    flex: 1 1 280px;
    max-width: 520px;
    margin: 0 auto;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--bg2);
    color: var(--fg2);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
  }
  .center:hover {
    border-color: var(--fg3);
  }
  .kbd {
    margin-left: auto;
    font-size: 11px;
    color: var(--fg3);
  }
  .actions {
    display: flex;
    gap: 8px;
    width: 300px;
    justify-content: flex-end;
  }
  .btn[disabled] {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .stop {
    width: 9px;
    height: 9px;
    border-radius: 1.5px;
    background: currentColor;
  }
</style>
