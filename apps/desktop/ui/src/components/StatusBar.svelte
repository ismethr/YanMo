<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { inTauri } from '../lib/backend';
  import { i18n, tr } from '../lib/i18n.svelte';
  import { updater } from '../lib/updater.svelte';
  import { prefs } from '../lib/prefs.svelte';

  const demo = !inTauri && new URLSearchParams(location.search).has('demo');

  let engineName = $derived(
    app.toolchain
      ? app.toolchain.tectonic
        ? 'Tectonic'
        : app.toolchain.latexmk
          ? 'latexmk · XeLaTeX'
          : tr('未找到编译引擎', 'No TeX engine found')
      : 'Tectonic · XeLaTeX',
  );
</script>

<footer>
  {#if app.project}
    <button onclick={() => app.showPanel('problems')} aria-label={tr('打开问题面板', 'Open problems panel')}>
      <span>✕ {app.errors}</span><span>⚠ {app.warnings}</span>
    </button>
    <button onclick={() => app.showPanel('log')} aria-label={tr('打开编译日志', 'Open build log')}>
      {#if app.compiling}
        {tr('编译中…', 'Compiling…')} {app.compileElapsed}s
        {#if app.compileElapsed >= 15 && app.lastCompile === null}<span class="muted">{tr('首次编译需要下载宏包，请稍候', 'the first build downloads packages, please wait')}</span>{/if}
      {:else if app.lastCompile}
        {app.lastCompile.success
          ? tr('✓ 已编译', '✓ Compiled')
          : app.lastCompile.pdf
            ? tr('⚠ 编译有错误', '⚠ Compiled with errors')
            : tr('✕ 编译失败', '✕ Compilation failed')} · {(app.lastCompile.duration_ms / 1000).toFixed(1)}s
      {:else}
        {engineName}
      {/if}
    </button>
  {/if}
  {#if app.project && prefs.draft}
    <button class="draft" title={tr('草稿模式：图片显示为方框。点击关闭', 'Draft mode: images appear as boxes. Click to turn off')} onclick={() => app.toggleDraft()}
      >{tr('草稿模式', 'Draft')}</button
    >
  {/if}
  {#if !inTauri && !demo}<span class="preview">{tr('浏览器预览模式 · 示例数据', 'Browser preview · sample data')}</span>{/if}
  <span class="grow"></span>
  {#if app.active && !app.active.startsWith('yanmo://')}
    <span>{tr(`行 ${app.cursor.line}，列 ${app.cursor.col}`, `Ln ${app.cursor.line}, Col ${app.cursor.col}`)}</span>
    <span>{app.isDirty(app.active) ? tr('未保存', 'Unsaved') : tr('已保存', 'Saved')}</span>
  {/if}
  {#if app.index}<span>{app.index.total_words.toLocaleString()} {app.wordUnit}</span>{/if}
  {#if updater.progress !== null}
    <span class="update">{tr('正在更新', 'Updating')} {Math.round(updater.progress * 100)}%</span>
  {:else if updater.available}
    <button class="update" title={tr('下载并安装，完成后自动重启', 'Download, install and restart')} onclick={() => void updater.install()}
      >{tr(`更新到 ${updater.available.version}`, `Update to ${updater.available.version}`)}</button
    >
  {/if}
  <span>UTF-8</span>
  <button
    class="lang"
    title={tr('切换界面语言', 'Switch interface language')}
    onclick={() => void app.switchLang(i18n.lang === 'zh' ? 'en' : 'zh')}>{i18n.lang === 'zh' ? '中文' : 'English'}</button
  >
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
  .draft {
    color: var(--warn);
    font-weight: 500;
  }
  .preview,
  .update {
    color: var(--accent);
  }
</style>
