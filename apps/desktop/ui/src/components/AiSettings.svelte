<script lang="ts">
  // AI 设置：服务商、模型、思考深度、接口地址；Key 存系统钥匙串，界面只显示"已设置"。
  import { ai } from '../lib/ai-store.svelte';
  import { runAi, saveAiSettings, setAiKey, type AiSettings } from '../lib/ai';
  import { app } from '../lib/store.svelte';
  import { tr } from '../lib/i18n.svelte';

  /** 嵌在设置窗口里时不显示"返回对话" */
  let { embedded = false }: { embedded?: boolean } = $props();

  const ANTHROPIC_MODELS = $derived([
    { id: 'claude-opus-5-5', name: tr('Claude Opus 5.5（推荐）', 'Claude Opus 5.5 (recommended)') },
    { id: 'claude-sonnet-5-5', name: tr('Claude Sonnet 5.5（更快、更便宜）', 'Claude Sonnet 5.5 (faster, cheaper)') },
    { id: 'claude-haiku-5-5', name: tr('Claude Haiku 5.5（最便宜）', 'Claude Haiku 5.5 (cheapest)') },
  ]);
  const EFFORTS = $derived([
    { id: 'low', name: tr('低：最快', 'Low: fastest') },
    { id: 'medium', name: tr('中：日常写作（默认）', 'Medium: everyday writing (default)') },
    { id: 'high', name: tr('高：复杂改写与审阅', 'High: complex rewrites and reviews') },
  ]);
  const PRESETS = $derived([
    { name: tr('Ollama（本地）', 'Ollama (local)'), url: 'http://localhost:11434/v1' },
    { name: 'DeepSeek', url: 'https://api.deepseek.com/v1' },
    { name: 'OpenAI', url: 'https://api.openai.com/v1' },
  ]);

  let s = $state<AiSettings>({ ...ai.info!.settings });
  let key = $state('');
  let saving = $state(false);
  let testResult = $state<string | null>(null);
  let hasKey = $derived(ai.info?.has_key[s.provider] ?? false);

  async function save() {
    saving = true;
    try {
      await saveAiSettings(s);
      if (key.trim()) await setAiKey(s.provider, key);
      key = '';
      await ai.loadSettings();
      app.flash(tr('AI 设置已保存', 'AI settings saved'));
    } catch (e) {
      app.flash(tr(`保存失败：${e}`, `Save failed: ${e}`));
    } finally {
      saving = false;
    }
  }

  async function removeKey() {
    await setAiKey(s.provider, '');
    await ai.loadSettings();
  }

  async function test() {
    await save();
    testResult = tr('连接中…', 'Connecting…');
    let reply = '';
    try {
      const r = await runAi({ kind: 'chat', history: [{ role: 'user', content: 'Reply with exactly: OK' }] }, {}, (t) => (reply += t), () => {})
        .done;
      testResult = tr(`✓ 连接成功（${r.outcome.model}）：`, `✓ Connected (${r.outcome.model}): `) + reply.trim().slice(0, 40);
    } catch (e) {
      testResult = `✕ ${String(e).replace(/^Error:\s*/, '')}`;
    }
  }
</script>

<div class="settings">
  <h3>{tr('AI 设置', 'AI Settings')}</h3>
  <p class="muted small">
    {tr(
      '研墨不提供模型服务，也不经手你的 Key：请求从本机直接发往你选择的服务商。',
      'YanMo does not provide a model service and never handles your key: requests go straight from this computer to the provider you choose.',
    )}
  </p>

  <fieldset>
    <legend>{tr('服务商', 'Provider')}</legend>
    <label><input type="radio" bind:group={s.provider} value="anthropic" /> Anthropic（Claude）</label>
    <label><input type="radio" bind:group={s.provider} value="openai_compatible" />
      {tr('OpenAI 兼容接口（DeepSeek、通义、Ollama…）', 'OpenAI-compatible API (DeepSeek, Qwen, Ollama…)')}</label
    >
  </fieldset>

  {#if s.provider === 'anthropic'}
    <label class="field">
      <span>{tr('模型', 'Model')}</span>
      <select bind:value={s.anthropic_model}>
        {#each ANTHROPIC_MODELS as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
      </select>
    </label>
    <label class="field">
      <span>{tr('思考深度', 'Thinking effort')}</span>
      <select bind:value={s.effort}>
        {#each EFFORTS as e (e.id)}<option value={e.id}>{e.name}</option>{/each}
      </select>
    </label>
  {:else}
    <label class="field">
      <span>{tr('接口地址', 'API URL')}</span>
      <input bind:value={s.openai_base_url} placeholder="https://api.deepseek.com/v1" />
    </label>
    <div class="presets">
      {#each PRESETS as p (p.url)}<button class="chip" onclick={() => (s.openai_base_url = p.url)}>{p.name}</button>{/each}
    </div>
    <label class="field">
      <span>{tr('模型名', 'Model name')}</span>
      <input bind:value={s.openai_model} placeholder={tr('如 deepseek-chat、qwen2.5:14b', 'e.g. deepseek-chat, qwen2.5:14b')} />
    </label>
  {/if}

  <label class="field">
    <span>API Key {#if hasKey}<span class="c-ok">{tr('· 已存入钥匙串', '· stored in the keychain')}</span>{/if}</span>
    <input type="password" bind:value={key} placeholder={hasKey
        ? tr('已设置；输入新 Key 可替换', 'Set. Enter a new key to replace it')
        : s.provider === 'anthropic'
          ? 'sk-ant-…'
          : tr('本地 Ollama 可留空', 'Leave empty for a local Ollama')} autocomplete="off" />
  </label>
  {#if hasKey}<button class="link" onclick={removeKey}>{tr('从钥匙串删除 Key', 'Remove key from keychain')}</button>{/if}

  <div class="actions">
    <button class="btn btn-pri" onclick={save} disabled={saving}>{tr('保存', 'Save')}</button>
    <button class="btn" onclick={test} disabled={saving}>{tr('测试连接', 'Test Connection')}</button>
    {#if ai.configured && !embedded}<button class="btn" onclick={() => (ai.showSettings = false)}>{tr('返回对话', 'Back to Chat')}</button>{/if}
  </div>
  {#if testResult}<p class="small">{testResult}</p>{/if}
</div>

<style>
  .settings {
    padding: 12px 14px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  .small {
    font-size: 12px;
    margin: 0;
    line-height: 1.6;
  }
  fieldset {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  legend {
    font-size: 12px;
    color: var(--fg2);
    padding: 0 4px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--fg2);
  }
  .field input,
  .field select {
    height: 28px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--fg);
    padding: 0 8px;
    font: inherit;
    font-size: 13px;
  }
  .presets {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .presets .chip {
    cursor: pointer;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .link {
    align-self: flex-start;
    font-size: 12px;
    color: var(--err);
  }
</style>
