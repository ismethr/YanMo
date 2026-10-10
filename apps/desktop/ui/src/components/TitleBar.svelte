<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
</script>

<header>
  <div class="brand">
    <img class="logo" src="/icon.svg" alt="" width="22" height="22" />
    <span>研墨</span>
  </div>
  <button class="center" onclick={() => (app.palette = true)}>
    <Icon name="search" size={14} />
    <span>{app.project ? `${app.project.name} — ` : ''}搜索命令、文件、label、文献</span>
    <span class="kbd">⇧⌘P</span>
  </button>
  <div class="actions">
    <button class="btn btn-pri" disabled={!app.project} onclick={() => app.compile()} title="编译（⌘B）">
      <Icon name="play" size={12} fill />{app.compiling ? '编译中…' : '编译'}
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
    width: 220px;
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
    width: 220px;
    justify-content: flex-end;
  }
  .btn[disabled] {
    opacity: 0.55;
    cursor: not-allowed;
  }
</style>
