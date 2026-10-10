<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { app } from '../lib/store.svelte';
  import { ai, REWRITE_LABELS, type AiMessage } from '../lib/ai-store.svelte';
  import { wordDiff } from '../lib/diff';
  import type { RewriteAction } from '../lib/ai';
  import Icon from './Icon.svelte';
  import AiSettingsView from './AiSettings.svelte';

  let draft = $state('');
  let list: HTMLDivElement | undefined = $state();
  const actions = Object.entries(REWRITE_LABELS) as [RewriteAction, string][];
  let hasSelection = $derived(!!app.selection?.text.trim());
  let compileErrors = $derived(app.lastCompile?.diagnostics.some((d) => d.severity === 'error') ?? false);

  onMount(() => {
    void ai.loadSettings();
  });

  // 新内容到达时滚到底部
  $effect(() => {
    void ai.messages.length;
    const last = ai.messages[ai.messages.length - 1];
    void (last && 'text' in last ? last.text : last && 'raw' in last ? last.raw : '');
    void tick().then(() => list?.scrollTo({ top: list.scrollHeight }));
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      void send();
    }
  }

  async function send() {
    const t = draft;
    draft = '';
    await ai.send(t);
  }

  function copy(text: string) {
    void navigator.clipboard.writeText(text).then(() => app.flash('已复制'));
  }

  type Proposal = Extract<AiMessage, { kind: 'proposal' }>;
  const statusText: Record<Proposal['status'], string> = {
    streaming: '生成中…',
    ready: '',
    accepted: '已替换原文 · ⌘Z 可撤销',
    rejected: '已放弃，原文未改动',
    stale: '原文已被修改，未替换',
    error: '',
  };
</script>

<aside aria-label="AI 助手">
  <div class="head">
    <span class="strong">AI 助手</span>
    {#if ai.info}
      <button class="model" onclick={() => (ai.showSettings = !ai.showSettings)} title="AI 设置">
        {ai.configured ? ai.modelName : '未配置'}
        <Icon name="settings" size={13} />
      </button>
    {/if}
    <button class="x" aria-label="清空对话" title="清空对话" onclick={() => ai.clear()} disabled={!!ai.running}>
      <Icon name="trash" size={14} />
    </button>
    <button class="x" aria-label="收起 AI 面板" onclick={() => (app.aiOpen = false)}><Icon name="close" size={14} /></button>
  </div>

  {#if ai.showSettings || (ai.info && !ai.configured)}
    <AiSettingsView />
  {:else}
    <div class="ctx">
      <span class="muted small">将附带：</span>
      {#each ai.contextChips as c (c)}<span class="chip">{c}</span>{:else}<span class="muted small">无（未打开文件）</span>{/each}
    </div>

    <div class="list" bind:this={list}>
      {#each ai.messages as m (m.id)}
        {#if m.kind === 'user'}
          <div class="user">{m.text}</div>
        {:else if m.kind === 'notice'}
          <div class="notice">{m.text}</div>
        {:else if m.kind === 'assistant'}
          <div class="assistant">
            {#if m.error}
              <p class="c-err">{m.error}</p>
            {:else}
              <div class="text">{m.text}{#if m.streaming}<span class="caret">▍</span>{/if}</div>
              {#if !m.streaming && m.text}
                <div class="meta">
                  <button onclick={() => copy(m.text)}>复制</button>
                  {#if m.model}<span class="muted">{m.model}</span>{/if}
                </div>
              {/if}
            {/if}
          </div>
        {:else}
          <div class="card">
            <div class="card-head">
              <span class="strong">{m.label}</span>
              <span class="muted small">{m.path.split('/').pop()}</span>
              {#if m.model}<span class="muted small right">{m.model}</span>{/if}
            </div>
            {#if m.status === 'error'}
              <p class="c-err pad">{m.error}</p>
            {:else if m.status === 'streaming'}
              <div class="diff mono">{m.raw.replace(/^```\w*\n?/, '').replace(/```\s*$/, '')}<span class="caret">▍</span></div>
            {:else if m.rewrite !== null}
              <div class="diff mono">
                {#each wordDiff(m.original, m.rewrite) as part, i (i)}<span class={part.kind}>{part.text}</span>{/each}
              </div>
            {/if}
            <div class="card-foot">
              {#if m.status === 'ready'}
                <button class="btn btn-pri" onclick={() => ai.accept(m)}>接受</button>
                <button class="btn" onclick={() => ai.reject(m)}>放弃</button>
                <button class="btn" onclick={() => m.rewrite && copy(m.rewrite)}>复制</button>
                <span class="muted small right">确认后才写入文件</span>
              {:else}
                <span class="small {m.status === 'accepted' ? 'c-ok' : 'muted'}">{statusText[m.status]}</span>
              {/if}
            </div>
          </div>
        {/if}
      {:else}
        <div class="empty muted">
          <p>选中编辑器里的文字，用下方按钮润色、翻译或缩写；也可以直接提问。</p>
          <p>所有改写都以对比形式给出，接受后才写入文件。</p>
        </div>
      {/each}
    </div>

    <div class="foot">
      <div class="qa" role="group" aria-label="选区操作">
        {#each actions as [id, label] (id)}
          <button class="qa-btn" disabled={!hasSelection || !!ai.running} title={hasSelection ? `${label}选中的文字` : '先在编辑器中选中文字'} onclick={() => ai.rewrite(id)}>{label}</button>
        {/each}
        {#if compileErrors}
          <button class="qa-btn" disabled={!!ai.running} onclick={() => ai.explainError()}>解释编译错误</button>
        {/if}
      </div>
      <div class="input">
        <label class="sr-only" for="ask">向 AI 提问</label>
        <textarea id="ask" rows="2" placeholder="问点什么（Enter 发送，Shift+Enter 换行）" bind:value={draft} onkeydown={onKey}></textarea>
        {#if ai.running}
          <button class="btn" onclick={() => ai.stop()}>停止</button>
        {:else}
          <button class="btn btn-pri" disabled={!draft.trim()} onclick={send}>发送</button>
        {/if}
      </div>
    </div>
  {/if}
</aside>

<style>
  aside {
    flex: 0 0 360px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg2);
    border-left: 1px solid var(--line);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px 8px 12px;
    border-bottom: 1px solid var(--line);
  }
  .strong {
    font-weight: 600;
  }
  .model {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--fg2);
    padding: 2px 6px;
    border-radius: 4px;
    max-width: 180px;
    overflow: hidden;
    white-space: nowrap;
  }
  .model:hover,
  .x:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .x {
    display: flex;
    padding: 3px;
    border-radius: 4px;
    color: var(--fg2);
  }
  .ctx {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
  }
  .small {
    font-size: 12px;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .user {
    align-self: flex-end;
    max-width: 88%;
    background: var(--sel);
    border-radius: 10px 10px 2px 10px;
    padding: 7px 11px;
    white-space: pre-wrap;
    line-height: 1.55;
  }
  .assistant .text {
    white-space: pre-wrap;
    line-height: 1.65;
    word-break: break-word;
  }
  .notice {
    font-size: 12px;
    color: var(--fg2);
    text-align: center;
  }
  .meta {
    display: flex;
    gap: 10px;
    margin-top: 4px;
    font-size: 11px;
  }
  .meta button {
    color: var(--fg2);
  }
  .meta button:hover {
    color: var(--fg);
  }
  .caret {
    animation: blink 1s steps(2) infinite;
    color: var(--fg3);
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .card {
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--bg);
    overflow: hidden;
  }
  .card-head,
  .card-foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
  }
  .card-head {
    border-bottom: 1px solid var(--line);
  }
  .card-foot {
    border-top: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .right {
    margin-left: auto;
  }
  .pad {
    padding: 8px 10px;
    margin: 0;
  }
  .diff {
    padding: 8px 10px;
    font-size: 12.5px;
    line-height: 1.7;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 340px;
    overflow: auto;
  }
  .diff .del {
    background: color-mix(in srgb, var(--err) 14%, transparent);
    color: var(--err);
    text-decoration: line-through;
  }
  .diff .add {
    background: color-mix(in srgb, var(--ok) 16%, transparent);
    color: var(--ok);
  }
  .empty {
    font-size: 12.5px;
    line-height: 1.7;
  }
  .foot {
    border-top: 1px solid var(--line);
    padding: 8px 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .qa {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .qa-btn {
    font-size: 12px;
    padding: 2px 9px;
    border-radius: 12px;
    border: 1px solid var(--line);
    color: var(--fg2);
  }
  .qa-btn:hover:not(:disabled) {
    color: var(--fg);
    border-color: var(--fg3);
  }
  .qa-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .input {
    display: flex;
    gap: 6px;
    align-items: flex-end;
  }
  textarea {
    flex: 1;
    resize: none;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--bg);
    padding: 6px 8px;
    line-height: 1.5;
  }
</style>
