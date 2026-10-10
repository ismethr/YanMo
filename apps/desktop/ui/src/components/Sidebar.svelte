<script lang="ts">
  import { app } from '../lib/store.svelte';
  import type { Asset, Section, TreeNode } from '../lib/types';
  import Icon from './Icon.svelte';

  let collapsed = $state<Record<string, boolean>>({});

  function badge(path: string) {
    const list = app.issuesFor(path);
    const errors = list.filter((i) => i.severity === 'error').length;
    return list.length ? { n: errors || list.length, err: errors > 0 } : null;
  }

  function openNode(n: TreeNode) {
    if (n.kind === 'dir') collapsed[n.path] = !collapsed[n.path];
    else if (n.kind === 'tex' || n.kind === 'bib' || n.kind === 'other') void app.openFile(n.path);
    else app.flash('图片预览将在后续版本支持');
  }

  const kindName: Record<Asset['kind'], string> = {
    figure: '图',
    table: '表',
    equation: '公式',
    algorithm: '算法',
    listing: '代码',
  };

  let bibQuery = $state('');
  let bib = $derived(
    (app.check?.bib ?? []).filter((e) => {
      const q = bibQuery.trim().toLowerCase();
      return !q || e.key.toLowerCase().includes(q) || (e.fields.title ?? '').toLowerCase().includes(q);
    }),
  );
  let undefinedCites = $derived(app.check?.issues.filter((i) => i.code === 'undefined_cite') ?? []);
  let target = 6000;
  let words = $derived(app.index?.total_words ?? 0);
</script>

{#snippet tree(nodes: TreeNode[], depth: number)}
  {#each nodes as n (n.path)}
    {@const b = badge(n.path)}
    <button class="row" class:on={app.active === n.path} style:padding-left="{8 + depth * 14}px" onclick={() => openNode(n)}>
      {#if n.kind === 'dir'}
        <span class="twist"><Icon name={collapsed[n.path] ? 'chevron' : 'chevronDown'} size={14} /></span>
      {:else}
        <span class="ext ext-{n.kind}">{n.kind === 'tex' ? 'TeX' : n.kind === 'bib' ? 'bib' : n.kind === 'image' ? 'img' : ''}</span>
      {/if}
      <span class="name">{n.name}</span>
      {#if app.isDirty(n.path)}<span class="dirty" title="未保存">●</span>{/if}
      {#if b}<span class="badge" class:err={b.err}>{b.n}</span>{/if}
    </button>
    {#if n.children && !collapsed[n.path]}{@render tree(n.children, depth + 1)}{/if}
  {/each}
{/snippet}

{#snippet outline(sections: Section[], depth: number)}
  {#each sections as s (s.id)}
    <button class="row" style:padding-left="{10 + depth * 14}px" onclick={() => app.openFile(s.span.file, s.span.line)}>
      <span class="name" class:strong={depth === 0}>{s.title}</span>
      <span class="count">{s.words} 字</span>
    </button>
    {@render outline(s.children, depth + 1)}
  {/each}
{/snippet}

<aside aria-label="侧栏">
  {#if app.activity === 'files'}
    <div class="sec-h">资源管理器 · {app.project?.name}</div>
    {@render tree(app.tree, 0)}
  {:else if app.activity === 'outline'}
    <div class="sec-h">论文大纲</div>
    {#if app.index}
      <div class="progress">
        <div class="muted pr"><span>全文 {words} 字</span><span>目标 {target.toLocaleString()}</span></div>
        <div class="bar"><div style:width="{Math.min(100, (words / target) * 100)}%"></div></div>
      </div>
      {#if app.index.front_words}<div class="row muted">（前置内容）<span class="count">{app.index.front_words} 字</span></div>{/if}
      {@render outline(app.index.outline, 0)}
      {#if !app.index.outline.length}<p class="empty">还没有章节。写下 \section{'{'}…{'}'} 后会出现在这里。</p>{/if}
    {:else}
      <p class="empty">没有找到主文件（含 \documentclass 的 .tex），无法生成大纲。</p>
    {/if}
  {:else if app.activity === 'assets'}
    <div class="sec-h">图表资产</div>
    {#each app.index?.assets ?? [] as a (a.span.file + a.span.start)}
      <button class="asset" onclick={() => app.openFile(a.span.file, a.span.line)}>
        <span class="thumb">{kindName[a.kind]}</span>
        <span class="meta">
          <span class="strong">{a.caption ?? kindName[a.kind]}</span>
          <span class="mono label">{a.label ?? '（无 label）'}</span>
          <span class={a.references.length ? 'muted' : 'c-warn'}>
            {a.references.length ? `被引用 ${a.references.length} 次` : '未被引用'}{a.files.length ? ` · ${a.files.join(', ')}` : ''}
          </span>
        </span>
      </button>
    {:else}
      <p class="empty">还没有 figure / table / 带 label 的公式。</p>
    {/each}
  {:else}
    <div class="sec-h">文献库</div>
    <div class="search">
      <label class="sr-only" for="bibq">搜索文献</label>
      <input id="bibq" placeholder="搜索 key 或标题" bind:value={bibQuery} />
    </div>
    {#each undefinedCites as u (u.key + String(u.line))}
      <button class="bibr" onclick={() => u.file && app.openFile(u.file, u.line ?? 1)}>
        <span class="bk"><span class="mono key">{u.key}</span><span class="c-err tag">未定义</span></span>
        <span class="muted small">{u.file}:{u.line} 引用了它，但文献库中没有</span>
      </button>
    {/each}
    {#each bib as e (e.span.file + e.key + e.span.start)}
      <button class="bibr" onclick={() => app.openFile(e.span.file, e.span.line)}>
        <span class="bk">
          <span class="mono key">{e.key}</span>
          <span class="tag {e.citations.length ? 'c-ok' : 'c-warn'}">{e.citations.length ? `引用 ${e.citations.length} 次` : '未使用'}</span>
        </span>
        <span class="title">{e.fields.title ?? '（无标题）'}</span>
        <span class="muted small">{[e.fields.author?.split(' and ')[0], e.fields.booktitle ?? e.fields.journal, e.fields.year].filter(Boolean).join(' · ')}</span>
      </button>
    {/each}
  {/if}
</aside>

<style>
  aside {
    flex: 0 0 264px;
    background: var(--bg2);
    border-right: 1px solid var(--line);
    overflow: auto;
    padding-bottom: 12px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .strong {
    font-weight: 500;
  }
  .twist {
    color: var(--fg3);
    display: flex;
  }
  .ext {
    width: 22px;
    font-family: var(--mono);
    font-size: 9.5px;
    font-weight: 600;
    flex: 0 0 22px;
  }
  .ext-tex {
    color: var(--tk-label);
  }
  .ext-bib {
    color: var(--tk-cite);
  }
  .ext-image {
    color: var(--tk-env);
  }
  .dirty {
    color: var(--fg2);
    font-size: 10px;
  }
  .badge {
    margin-left: auto;
    font-size: 11px;
    font-weight: 600;
    min-width: 16px;
    text-align: center;
    border-radius: 8px;
    padding: 0 5px;
    color: #fff;
    background: var(--warn);
  }
  .badge.err {
    background: var(--err);
  }
  .count {
    margin-left: auto;
    font-size: 12px;
    color: var(--fg2);
  }
  .progress {
    padding: 0 12px 10px;
  }
  .pr {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--line);
    margin-top: 6px;
    overflow: hidden;
  }
  .bar div {
    height: 4px;
    background: var(--accent);
  }
  .empty {
    margin: 8px 12px;
    color: var(--fg2);
    font-size: 12px;
    line-height: 1.6;
  }
  .asset {
    display: flex;
    gap: 10px;
    width: 100%;
    padding: 8px 12px;
  }
  .asset:hover,
  .bibr:hover {
    background: var(--hover);
  }
  .thumb {
    flex: 0 0 52px;
    height: 40px;
    border-radius: 4px;
    border: 1px solid var(--line);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    color: var(--fg2);
    background: var(--bg);
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    font-size: 12px;
  }
  .label {
    color: var(--tk-label);
    font-size: 11px;
  }
  .search {
    padding: 0 12px 8px;
  }
  .search input {
    width: 100%;
    height: 28px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--bg);
    padding: 0 8px;
    font-size: 12px;
  }
  .bibr {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    padding: 8px 12px;
  }
  .bk {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .key {
    font-size: 12px;
    color: var(--tk-cite);
  }
  .tag {
    margin-left: auto;
    font-size: 11px;
  }
  .title {
    line-height: 1.4;
  }
  .small {
    font-size: 12px;
  }
</style>
