<script lang="ts">
  import { app, type Activity } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import { tr } from '../lib/i18n.svelte';

  const items: { id: Activity; icon: string; label: string }[] = $derived([
    { id: 'files', icon: 'files', label: tr('文件', 'Files') },
    { id: 'outline', icon: 'outline', label: tr('大纲', 'Outline') },
    { id: 'assets', icon: 'assets', label: tr('图表资产', 'Figures & Tables') },
    { id: 'bib', icon: 'bib', label: tr('文献', 'Bibliography') },
  ]);
</script>

<nav aria-label={tr('活动栏', 'Activity bar')}>
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
