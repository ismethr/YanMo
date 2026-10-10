<script lang="ts">
  // AI 设置：服务商、模型、思考深度、接口地址；Key 存系统钥匙串，界面只显示"已设置"。
  import { ai } from '../lib/ai-store.svelte';
  import { runAi, saveAiSettings, setAiKey, type AiSettings } from '../lib/ai';
  import { app } from '../lib/store.svelte';

  const ANTHROPIC_MODELS = [
    { id: 'claude-opus-5-5', name: 'Claude Opus 5.5（推荐）' },
    { id: 'claude-sonnet-5-5', name: 'Claude Sonnet 5.5（更快、更便宜）' },
    { id: 'claude-haiku-5-5', name: 'Claude Haiku 5.5（最便宜）' },
  ];
  const EFFORTS = [
    { id: 'low', name: '低：最快' },
    { id: 'medium', name: '中：日常写作（默认）' },
    { id: 'high', name: '高：复杂改写与审阅' },
  ];
  const PRESETS = [
    { name: 'Ollama（本地）', url: 'http://localhost:11434/v1' },
    { name: 'DeepSeek', url: 'https://api.deepseek.com/v1' },
    { name: 'OpenAI', url: 'https://api.openai.com/v1' },
  ];

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
      app.flash('AI 设置已保存');
    } catch (e) {
      app.flash(`保存失败：${e}`);
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
    testResult = '连接中…';
    let reply = '';
    try {
      const r = await runAi({ kind: 'chat', history: [{ role: 'user', content: 'Reply with exactly: OK' }] }, {}, (t) => (reply += t), () => {})
        .done;
      testResult = `✓ 连接成功（${r.outcome.model}）：${reply.trim().slice(0, 40)}`;
    } catch (e) {
      testResult = `✕ ${String(e).replace(/^Error:\s*/, '')}`;
    }
  }
</script>

<div class="settings">
  <h3>AI 设置</h3>
  <p class="muted small">研墨不提供模型服务，也不经手你的 Key：请求从本机直接发往你选择的服务商。</p>

  <fieldset>
    <legend>服务商</legend>
    <label><input type="radio" bind:group={s.provider} value="anthropic" /> Anthropic（Claude）</label>
    <label><input type="radio" bind:group={s.provider} value="openai_compatible" /> OpenAI 兼容接口（DeepSeek、通义、Ollama…）</label>
  </fieldset>

  {#if s.provider === 'anthropic'}
    <label class="field">
      <span>模型</span>
      <select bind:value={s.anthropic_model}>
        {#each ANTHROPIC_MODELS as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
      </select>
    </label>
    <label class="field">
      <span>思考深度</span>
      <select bind:value={s.effort}>
        {#each EFFORTS as e (e.id)}<option value={e.id}>{e.name}</option>{/each}
      </select>
    </label>
  {:else}
    <label class="field">
      <span>接口地址</span>
      <input bind:value={s.openai_base_url} placeholder="https://api.deepseek.com/v1" />
    </label>
    <div class="presets">
      {#each PRESETS as p (p.url)}<button class="chip" onclick={() => (s.openai_base_url = p.url)}>{p.name}</button>{/each}
    </div>
    <label class="field">
      <span>模型名</span>
      <input bind:value={s.openai_model} placeholder="如 deepseek-chat、qwen2.5:14b" />
    </label>
  {/if}

  <label class="field">
    <span>API Key {#if hasKey}<span class="c-ok">· 已存入钥匙串</span>{/if}</span>
    <input type="password" bind:value={key} placeholder={hasKey ? '已设置；输入新 Key 可替换' : s.provider === 'anthropic' ? 'sk-ant-…' : '本地 Ollama 可留空'} autocomplete="off" />
  </label>
  {#if hasKey}<button class="link" onclick={removeKey}>从钥匙串删除 Key</button>{/if}

  <div class="actions">
    <button class="btn btn-pri" onclick={save} disabled={saving}>保存</button>
    <button class="btn" onclick={test} disabled={saving}>测试连接</button>
    {#if ai.configured}<button class="btn" onclick={() => (ai.showSettings = false)}>返回对话</button>{/if}
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
