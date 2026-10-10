<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
</script>

<aside aria-label="AI 助手">
  <div class="head">
    <span class="strong">AI 助手</span>
    <button class="x" aria-label="收起 AI 面板" onclick={() => (app.aiOpen = false)}><Icon name="close" size={14} /></button>
  </div>
  <div class="body">
    <p>AI 功能将在 M3 接入：</p>
    <ul class="muted">
      <li>自带 Key：Claude、OpenAI 兼容接口、本地 Ollama</li>
      <li>选中文本润色、翻译、缩写</li>
      <li>解释编译错误、修复引用问题</li>
      <li>所有修改以 diff 呈现，接受后才写入文件</li>
    </ul>
  </div>
</aside>

<style>
  aside {
    flex: 0 0 320px;
    display: flex;
    flex-direction: column;
    background: var(--bg2);
    border-left: 1px solid var(--line);
  }
  .head {
    display: flex;
    align-items: center;
    padding: 9px 12px;
    border-bottom: 1px solid var(--line);
  }
  .strong {
    font-weight: 600;
  }
  .x {
    margin-left: auto;
    display: flex;
    color: var(--fg2);
  }
  .body {
    padding: 12px 14px;
    line-height: 1.7;
  }
  ul {
    padding-left: 18px;
    margin: 4px 0;
  }
</style>
