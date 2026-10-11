<script lang="ts">
  // 设置窗口（⌘, 或活动栏底部的齿轮）。应用偏好记在本机；编译引擎等项目设置写入 .yanmo/project.toml。
  import { onMount } from 'svelte';
  import { app } from '../lib/store.svelte';
  import { ai } from '../lib/ai-store.svelte';
  import { i18n, tr } from '../lib/i18n.svelte';
  import { prefs, savePrefs } from '../lib/prefs.svelte';
  import { updater } from '../lib/updater.svelte';
  import { inTauri } from '../lib/backend';
  import AiSettingsView from './AiSettings.svelte';
  import pkg from '../../package.json';

  type Section = 'general' | 'editor' | 'compile' | 'ai' | 'agents' | 'about';
  let section = $state<Section>('general');
  let sections = $derived<{ id: Section; label: string }[]>([
    { id: 'general', label: tr('通用', 'General') },
    { id: 'editor', label: tr('编辑器', 'Editor') },
    { id: 'compile', label: tr('编译', 'Compile') },
    { id: 'ai', label: 'AI' },
    { id: 'agents', label: tr('Agent 接入', 'Agents') },
    { id: 'about', label: tr('关于', 'About') },
  ]);

  onMount(() => {
    void app.loadProjectSettings();
    void ai.loadSettings();
  });

  function close() {
    app.settingsOpen = false;
  }

  /** 修改偏好并保存 */
  function set<K extends keyof typeof prefs>(key: K, value: (typeof prefs)[K]) {
    prefs[key] = value;
    savePrefs();
  }

  let tc = $derived(app.toolchain);
  let engines = $derived([
    { id: 'auto', label: tr('自动选择（推荐）', 'Automatic (recommended)'), ok: true },
    { id: 'tectonic', label: tr('Tectonic（内置，XeTeX）', 'Tectonic (built in, XeTeX)'), ok: !!tc?.tectonic },
    { id: 'xelatex', label: 'XeLaTeX (latexmk)', ok: !!tc?.latexmk },
    { id: 'pdflatex', label: 'pdfLaTeX (latexmk)', ok: !!tc?.latexmk },
    { id: 'lualatex', label: 'LuaLaTeX (latexmk)', ok: !!tc?.latexmk },
  ] as const);

  async function openUrl(url: string) {
    if (inTauri) {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(url);
    } else {
      window.open(url, '_blank', 'noopener');
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<div class="backdrop" role="presentation" onclick={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={tr('设置', 'Settings')} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
    <nav aria-label={tr('设置分类', 'Settings sections')}>
      <h2>{tr('设置', 'Settings')}</h2>
      {#each sections as s (s.id)}
        <button class:on={section === s.id} onclick={() => (section = s.id)}>{s.label}</button>
      {/each}
    </nav>
    <div class="pane">
      <button class="x" aria-label={tr('关闭', 'Close')} title={tr('关闭（Esc）', 'Close (Esc)')} onclick={close}>×</button>

      {#if section === 'general'}
        <h3>{tr('通用', 'General')}</h3>
        <label class="row">
          <span>{tr('界面语言', 'Interface language')}</span>
          <select value={i18n.lang} onchange={(e) => void app.switchLang((e.currentTarget as HTMLSelectElement).value as 'zh' | 'en')}>
            <option value="zh">简体中文</option>
            <option value="en">English</option>
          </select>
        </label>
        <label class="row">
          <span>{tr('启动时检查更新', 'Check for updates on launch')}</span>
          <input type="checkbox" checked={prefs.checkUpdates} onchange={(e) => set('checkUpdates', (e.currentTarget as HTMLInputElement).checked)} />
        </label>
        <div class="row">
          <span class="muted">{tr(`当前版本 ${pkg.version}`, `Version ${pkg.version}`)}</span>
          <button class="btn" disabled={updater.checking} onclick={() => void updater.check()}>{updater.checking ? tr('检查中…', 'Checking…') : tr('检查更新', 'Check for Updates')}</button>
        </div>
      {:else if section === 'editor'}
        <h3>{tr('编辑器', 'Editor')}</h3>
        <label class="row">
          <span>{tr('字号', 'Font size')}</span>
          <select value={String(prefs.fontSize)} onchange={(e) => set('fontSize', Number((e.currentTarget as HTMLSelectElement).value))}>
            {#each [12, 13, 13.5, 14, 15, 16, 18, 20] as n (n)}<option value={String(n)}>{n}px</option>{/each}
          </select>
        </label>
        <label class="row">
          <span>{tr('长行自动换行', 'Wrap long lines')}</span>
          <input type="checkbox" checked={prefs.wrap} onchange={(e) => set('wrap', (e.currentTarget as HTMLInputElement).checked)} />
        </label>
        <label class="row">
          <span>
            {tr('停止输入后自动保存', 'Save automatically after typing stops')}
            <small class="muted">{tr('关闭后用 ⌘S 保存', 'When off, save with ⌘S')}</small>
          </span>
          <input type="checkbox" checked={prefs.autosave} onchange={(e) => set('autosave', (e.currentTarget as HTMLInputElement).checked)} />
        </label>
      {:else if section === 'compile'}
        <h3>{tr('编译', 'Compile')}</h3>
        {#if app.project && app.projectSettings}
          <p class="group">{tr(`本项目（${app.project.name}）· 保存在 .yanmo/project.toml`, `This project (${app.project.name}) · saved in .yanmo/project.toml`)}</p>
          <label class="row">
            <span>{tr('编译引擎', 'Engine')}</span>
            <select
              value={app.projectSettings.engine}
              onchange={(e) => void app.updateProjectSettings({ engine: (e.currentTarget as HTMLSelectElement).value as 'auto' })}
            >
              {#each engines as en (en.id)}
                <option value={en.id} disabled={!en.ok}>{en.label}{en.ok ? '' : tr('（未安装）', ' (not installed)')}</option>
              {/each}
            </select>
          </label>
          <p class="hint muted">
            {tr(
              '自动选择：主文件开头有 % !TEX program = pdflatex / lualatex 时用 latexmk，否则用内置的 Tectonic。',
              'Automatic: uses latexmk when the main file starts with % !TEX program = pdflatex / lualatex, otherwise the built-in Tectonic.',
            )}
          </p>
          <label class="row">
            <span>
              {tr('允许 shell-escape', 'Allow shell-escape')}
              <small class="muted">{tr('minted、svg 等宏包需要；会允许 TeX 执行外部命令，只对信任的项目开启', 'Needed by minted, svg and similar packages; lets TeX run external commands, so enable it only for projects you trust')}</small>
            </span>
            <input type="checkbox" checked={app.projectSettings.shell_escape} onchange={(e) => void app.updateProjectSettings({ shell_escape: (e.currentTarget as HTMLInputElement).checked })} />
          </label>
          <div class="row">
            <span>{tr('主文件', 'Main file')}</span>
            <span class="mono muted">{app.projectSettings.main ?? tr('未找到', 'not found')}</span>
          </div>
          <div class="row">
            <span>
              {tr('编译缓存', 'Build files')}
              <small class="muted">{tr('.yanmo/build/ 中的中间文件与 PDF；编译出现奇怪的问题时可以清理后重试', 'Intermediate files and the PDF in .yanmo/build/; clean them if a build behaves oddly')}</small>
            </span>
            <button class="btn" onclick={() => void app.cleanBuild()}>{tr('清理', 'Clean')}</button>
          </div>
        {:else}
          <p class="muted hint">{tr('打开论文后可设置本项目的编译引擎。', 'Open a paper to choose its engine.')}</p>
        {/if}

        <p class="group">{tr('所有项目', 'All projects')}</p>
        <label class="row">
          <span>{tr('保存后自动编译', 'Compile on save')}</span>
          <input type="checkbox" checked={prefs.autoCompile} onchange={(e) => set('autoCompile', (e.currentTarget as HTMLInputElement).checked)} />
        </label>
        <label class="row">
          <span>
            {tr('草稿模式', 'Draft mode')}
            <small class="muted">{tr('不载入图片（显示为同尺寸的方框），编译更快、PDF 更小；定稿前请关闭', 'Skips images (drawn as boxes of the same size) for faster builds and a smaller PDF; turn it off before the final build')}</small>
          </span>
          <input type="checkbox" checked={prefs.draft} onchange={() => app.toggleDraft()} />
        </label>
        <label class="row">
          <span>
            {tr('编译超时', 'Timeout')}
            <small class="muted">{tr('超过这个时间仍未结束就停止编译；首次用 Tectonic 编译需要下载宏包，可能要几分钟', 'Stops a build that runs longer; the first Tectonic build downloads packages and may take a few minutes')}</small>
          </span>
          <select value={String(prefs.compileTimeout)} onchange={(e) => set('compileTimeout', Number((e.currentTarget as HTMLSelectElement).value))}>
            {#each [60, 120, 300, 600, 0] as n (n)}<option value={String(n)}>{n ? tr(`${n / 60} 分钟`, `${n / 60} min`) : tr('不限', 'None')}</option>{/each}
          </select>
        </label>

        <p class="group">{tr('编译工具', 'Tools')}</p>
        <dl class="tools">
          {#each [['Tectonic', tc?.tectonic], ['latexmk', tc?.latexmk], ['Pandoc', tc?.pandoc], ['Ghostscript', tc?.ghostscript]] as [name, path] (name)}
            <dt>{name}</dt>
            <dd class:c-ok={!!path} class="mono">{path ?? tr('未找到', 'not found')}</dd>
          {/each}
        </dl>
        <p class="hint muted">
          {tr(
            '内置 Tectonic 即可编译；需要 pdfLaTeX / LuaLaTeX 时安装 TeX Live 或 MacTeX，导出 Word 需要 Pandoc。安装后点"重新检测"。',
            'The built-in Tectonic is enough to compile. Install TeX Live or MacTeX for pdfLaTeX / LuaLaTeX, and Pandoc for Word export, then click Detect again.',
          )}
        </p>
        <div><button class="btn" onclick={() => void app.redetectToolchain()}>{tr('重新检测', 'Detect again')}</button></div>
      {:else if section === 'ai'}
        {#if ai.info}
          <div class="ai"><AiSettingsView embedded /></div>
        {:else}
          <p class="muted">{tr('加载中…', 'Loading…')}</p>
        {/if}
      {:else if section === 'agents'}
        <h3>{tr('接入 Claude Code 等 Agent', 'Connect Claude Code and other agents')}</h3>
        <p class="hint">
          {tr(
            '研墨附带的 ym 命令行提供 MCP Server：Agent 可以读取大纲、检查引用、编译并读取错误、导入文献、调整章节。改稿由 Agent 自己的文件工具完成，并由你在 Agent 中确认。',
            'The ym command line that ships with YanMo includes an MCP server: agents can read the outline, check references, compile and read errors, import references and reorder sections. Edits are made with the agent’s own file tools, which you approve in the agent.',
          )}
        </p>
        <div>
          <button class="btn btn-pri" disabled={!app.project} onclick={() => void app.copyMcpCommand()}>{tr('复制 Claude Code 接入命令', 'Copy Claude Code setup command')}</button>
          {#if !app.project}<span class="muted small">{tr('（先打开论文）', ' (open a paper first)')}</span>{/if}
        </div>
        <p class="hint muted">{tr('在终端中运行复制的命令即可。其他客户端：命令为 ym，参数为 mcp -C <论文目录>。', 'Run the copied command in a terminal. For other clients, the command is ym with the arguments mcp -C <paper folder>.')}</p>
      {:else}
        <h3>{tr('关于研墨', 'About YanMo')}</h3>
        <p>{tr(`研墨 YanMo ${pkg.version} · 本地优先的 AI LaTeX 论文工作台`, `YanMo ${pkg.version} · a local-first AI LaTeX workspace for papers`)}</p>
        <p class="muted hint">{tr('开源软件，MIT 或 Apache-2.0 双许可。', 'Open source, dual-licensed under MIT or Apache-2.0.')}</p>
        <div class="links">
          <button class="btn" onclick={() => void openUrl(i18n.lang === 'zh' ? 'https://yanmo.ink/' : 'https://yanmo.ink/en/')}>{tr('官网', 'Website')}</button>
          <button class="btn" onclick={() => void openUrl('https://github.com/ismethr/YanMo')}>GitHub</button>
          <button class="btn" onclick={() => void openUrl('https://github.com/ismethr/YanMo/issues')}>{tr('反馈问题', 'Report a problem')}</button>
          <button class="btn" onclick={() => void openUrl('https://github.com/ismethr/YanMo/blob/main/CHANGELOG.md')}>{tr('更新日志', 'Changelog')}</button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.2);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 40;
  }
  .dialog {
    position: relative;
    display: flex;
    width: min(760px, 94vw);
    height: min(560px, 88vh);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.22);
    overflow: hidden;
  }
  nav {
    flex: 0 0 168px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 14px 8px;
    background: var(--bg2);
    border-right: 1px solid var(--line);
  }
  nav h2 {
    font-size: 13px;
    margin: 2px 10px 10px;
  }
  nav button {
    text-align: left;
    padding: 6px 10px;
    border-radius: 6px;
  }
  nav button:hover {
    background: var(--hover);
  }
  nav button.on {
    background: var(--sel);
    font-weight: 500;
  }
  .pane {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 18px 22px 22px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .x {
    position: absolute;
    top: 8px;
    right: 10px;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    font-size: 18px;
    color: var(--fg2);
  }
  .x:hover {
    background: var(--hover);
  }
  h3 {
    margin: 0 0 4px;
    font-size: 15px;
  }
  .group {
    margin: 10px 0 0;
    font-size: 11px;
    font-weight: 600;
    color: var(--fg2);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 30px;
  }
  .row > span:first-child {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  small {
    font-size: 11.5px;
    line-height: 1.5;
  }
  select {
    height: 28px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--fg);
    padding: 0 8px;
    font: inherit;
    max-width: 260px;
  }
  input[type='checkbox'] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
    flex: 0 0 auto;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    line-height: 1.6;
  }
  .tools {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 14px;
    margin: 0;
    font-size: 12px;
  }
  .tools dt {
    color: var(--fg2);
  }
  .tools dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .links {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .ai {
    margin: -12px -14px;
  }
  .small {
    font-size: 12px;
  }
</style>
