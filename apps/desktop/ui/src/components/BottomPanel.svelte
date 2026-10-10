<script lang="ts">
  import { app } from '../lib/store.svelte';
</script>

<section aria-label="面板">
  <div class="head" role="tablist">
    <button role="tab" aria-selected={app.bottom === 'problems'} class:on={app.bottom === 'problems'} onclick={() => (app.bottom = 'problems')}>
      问题 <span class="chip">{app.check?.issues.length ?? 0}</span>
    </button>
    <button role="tab" aria-selected={app.bottom === 'log'} class:on={app.bottom === 'log'} onclick={() => (app.bottom = 'log')}>编译日志</button>
    <span class="hint muted">与 <span class="mono">ym check</span> 同源</span>
  </div>
  <div class="body">
    {#if app.bottom === 'problems'}
      {#each app.check?.issues ?? [] as p, i (i)}
        <button class="prob" onclick={() => p.file && app.openFile(p.file, p.line ?? 1)}>
          <span class={p.severity === 'error' ? 'c-err' : 'c-warn'} aria-label={p.severity === 'error' ? '错误' : '警告'}>{p.severity === 'error' ? '✕' : '!'}</span>
          <span class="msg">{p.message}</span>
          <span class="mono code">{p.code}</span>
          <span class="loc">{p.file ?? ''}{p.line ? `:${p.line}` : ''}</span>
        </button>
      {:else}
        <p class="c-ok pad">{app.check ? '未发现问题' : '尚未检查（项目缺少主文件？）'}</p>
      {/each}
    {:else}
      <p class="muted pad mono">编译功能将在 M1 接入（内置 Tectonic）。</p>
    {/if}
  </div>
</section>

<style>
  section {
    flex: 0 0 200px;
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
  .hint {
    margin-left: auto;
    font-size: 12px;
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
</style>
