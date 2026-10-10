<script lang="ts">
  import { app, type Activity } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  const items: { id: Activity; icon: string; label: string }[] = [
    { id: 'files', icon: 'files', label: '文件' },
    { id: 'outline', icon: 'outline', label: '大纲' },
    { id: 'assets', icon: 'assets', label: '图表资产' },
    { id: 'bib', icon: 'bib', label: '文献' },
  ];
</script>

<nav aria-label="活动栏">
  {#each items as it (it.id)}
    <button class:on={app.activity === it.id} aria-label={it.label} title={it.label} onclick={() => (app.activity = it.id)}>
      <Icon name={it.icon} size={22} />
    </button>
  {/each}
</nav>

<style>
  nav {
    flex: 0 0 48px;
    display: flex;
    flex-direction: column;
    background: var(--bg3);
    border-right: 1px solid var(--line);
  }
  button {
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg3);
    position: relative;
  }
  button:hover,
  button.on {
    color: var(--fg);
  }
  button.on::before {
    content: '';
    position: absolute;
    left: 0;
    top: 10px;
    bottom: 10px;
    width: 2px;
    background: var(--accent);
  }
</style>
