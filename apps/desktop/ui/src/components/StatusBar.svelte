<script lang="ts">
  import { app, PDF_TAB } from '../lib/store.svelte';
  import { inTauri } from '../lib/backend';

  const demo = !inTauri && new URLSearchParams(location.search).has('demo');

  let engineName = $derived(
    app.toolchain
      ? app.toolchain.tectonic
        ? 'Tectonic'
        : app.toolchain.latexmk
          ? 'latexmk · XeLaTeX'
          : '未找到编译引擎'
      : 'Tectonic · XeLaTeX',
  );
</script>

<footer>
  {#if app.project}
    <button onclick={() => (app.bottom = 'problems')} aria-label="打开问题面板">
      <span>✕ {app.errors}</span><span>⚠ {app.warnings}</span>
    </button>
    <button onclick={() => (app.bottom = 'log')} aria-label="打开编译日志">
      {#if app.compiling}
        编译中…
      {:else if app.lastCompile}
        {app.lastCompile.success ? '✓ 已编译' : app.lastCompile.pdf ? '⚠ 编译有错误' : '✕ 编译失败'} · {(app.lastCompile.duration_ms / 1000).toFixed(1)}s
      {:else}
        {engineName}
      {/if}
    </button>
  {/if}
  {#if !inTauri && !demo}<span class="preview">浏览器预览模式 · 示例数据</span>{/if}
  <span class="grow"></span>
  {#if app.active && app.active !== PDF_TAB}
    <span>行 {app.cursor.line}，列 {app.cursor.col}</span>
    <span>{app.isDirty(app.active) ? '未保存' : '已保存'}</span>
  {/if}
  {#if app.index}<span>{app.index.total_words.toLocaleString()} 字</span>{/if}
  <span>UTF-8</span>
</footer>

<style>
  footer {
    display: flex;
    align-items: center;
    height: 24px;
    flex: 0 0 24px;
    background: var(--sb);
    color: var(--sbfg);
    border-top: 1px solid var(--line);
    font-size: 12px;
  }
  footer > * {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    height: 100%;
  }
  button:hover {
    background: var(--hover);
  }
  .grow {
    flex: 1;
  }
  .preview {
    color: var(--accent);
  }
</style>
