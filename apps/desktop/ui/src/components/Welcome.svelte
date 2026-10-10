<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
</script>

<div class="welcome">
  <div class="inner">
    <div class="hero">
      <img class="logo" src="/icon.svg" alt="" width="72" height="72" />
      <div>
        <h1>研墨 <span class="muted">YanMo</span></h1>
        <p class="muted">本地优先的 AI LaTeX 论文工作台 · 结构清晰，引用可靠，修改可控</p>
      </div>
    </div>

    <div class="cols">
      <section>
        <h2>开始</h2>
        <button class="link" onclick={() => app.openFolder()}><Icon name="folder" />打开文件夹…</button>
        <span class="link off"><Icon name="files" />从模板新建论文（即将推出）</span>
      </section>
      <section>
        <h2>最近</h2>
        {#each app.recent as r (r)}
          <button class="link recent" onclick={() => app.openFolder(r)}>
            <span>{r.split(/[\\/]/).filter(Boolean).pop()}</span><span class="muted path">{r}</span>
          </button>
        {:else}
          <p class="muted">还没有打开过项目。</p>
        {/each}
      </section>
    </div>
  </div>
</div>

<style>
  .welcome {
    flex: 1;
    overflow: auto;
  }
  .inner {
    max-width: 960px;
    margin: 0 auto;
    padding: 72px 32px;
    display: flex;
    flex-direction: column;
    gap: 44px;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 20px;
  }
  h1 {
    margin: 0;
    font-size: 32px;
    font-weight: 500;
  }
  h1 .muted {
    font-weight: 400;
  }
  .hero p {
    margin: 6px 0 0;
    font-size: 15px;
  }
  .cols {
    display: flex;
    flex-wrap: wrap;
    gap: 56px;
  }
  section {
    flex: 1 1 320px;
  }
  h2 {
    margin: 0 0 10px;
    font-size: 18px;
    font-weight: 500;
  }
  .link {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 0;
    font-size: 14px;
    color: var(--tk-cmd);
  }
  .link:hover {
    text-decoration: underline;
  }
  .link.off {
    color: var(--fg3);
  }
  .link.off:hover {
    text-decoration: none;
  }
  .recent {
    gap: 14px;
    align-items: baseline;
  }
  .path {
    font-size: 12px;
  }
</style>
