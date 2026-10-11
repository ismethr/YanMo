<script lang="ts">
  import { tick } from 'svelte';
  import { app } from '../lib/store.svelte';
  import type { Asset, TreeNode } from '../lib/types';
  import Icon from './Icon.svelte';
  import Thumb from './Thumb.svelte';
  import { tr } from '../lib/i18n.svelte';

  // 文件夹默认折叠（图片等资源目录往往很长），点开后记住
  let expanded = $state<Record<string, boolean>>({});

  function badge(path: string) {
    const list = app.issuesFor(path);
    const errors = list.filter((i) => i.severity === 'error').length;
    return list.length ? { n: errors || list.length, err: errors > 0 } : null;
  }

  function openNode(n: TreeNode) {
    if (n.kind === 'dir') expanded[n.path] = !expanded[n.path];
    else void app.openFile(n.path);
  }

  let kindName: Record<Asset['kind'], string> = $derived({
    figure: tr('图', 'Figure'),
    table: tr('表', 'Table'),
    equation: tr('公式', 'Equation'),
    algorithm: tr('算法', 'Algorithm'),
    listing: tr('代码', 'Listing'),
  });
  // 分组标题（英文用复数）
  let kindHead: Record<Asset['kind'], string> = $derived({
    figure: tr('图', 'Figures'),
    table: tr('表', 'Tables'),
    equation: tr('公式', 'Equations'),
    algorithm: tr('算法', 'Algorithms'),
    listing: tr('代码', 'Listings'),
  });

  // 图表资产按类型分组并编号（图、表按出现顺序编号，与 LaTeX 默认一致；公式只列带 label 的）
  const kindOrder: Asset['kind'][] = ['figure', 'table', 'algorithm', 'listing', 'equation'];
  let groups = $derived(
    kindOrder
      .map((kind) => ({
        kind,
        items: (app.index?.assets ?? [])
          .filter((a) => a.kind === kind)
          .map((a, i) => ({ ...a, no: kind === 'equation' ? '' : `${kindName[kind]} ${i + 1}` })),
      }))
      .filter((g) => g.items.length),
  );
  let assetFolded = $state<Record<string, boolean>>({ equation: true });

  /** 把 BibTeX 字段转成可读文本：去掉保护大小写的花括号、常见转义与破折号 */
  function tex(s: string | undefined): string {
    return (s ?? '')
      .replace(/\\(?:textit|emph|textbf|mathrm)\s*/g, '')
      .replace(/\\([&%$#_])/g, '$1')
      .replace(/[{}]/g, '')
      .replace(/---/g, '—')
      .replace(/--/g, '–')
      .replace(/~/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
  }

  function authors(raw: string | undefined): string {
    const list = tex(raw).split(/\s+and\s+/i).filter(Boolean);
    if (!list.length) return '';
    const last = (a: string) => (a.includes(',') ? a.split(',')[0] : a.split(' ').pop()!).trim();
    if (list.length === 1) return last(list[0]);
    if (list.length === 2 && !/others/i.test(list[1])) return `${last(list[0])} & ${last(list[1])}`;
    return `${last(list[0])} et al.`;
  }

  let bibQuery = $state('');
  let importId = $state('');
  async function doImport() {
    if (await app.importBib(importId)) importId = '';
  }
  let bibFilter = $state<'all' | 'unused' | 'review'>('all');
  // 需要人工核对：信息不一致或没找到登记记录
  const needsReview = (key: string) => ['mismatch', 'not_found'].includes(app.bibVerify[key]?.status ?? '');
  let reviewCount = $derived((app.check?.bib ?? []).filter((e) => needsReview(e.key)).length);
  let verified = $derived(Object.keys(app.bibVerify).length > 0);
  const vmark: Record<string, { icon: string; cls: string }> = {
    ok: { icon: '✓', cls: 'c-ok' },
    resolved: { icon: '✓', cls: 'muted' },
    mismatch: { icon: '!', cls: 'c-warn' },
    not_found: { icon: '?', cls: 'muted' },
    skipped: { icon: '–', cls: 'muted' },
    error: { icon: '✕', cls: 'c-err' },
  };
  let unusedCount = $derived((app.check?.bib ?? []).filter((e) => e.unused).length);
  let bib = $derived(
    (app.check?.bib ?? []).filter((e) => {
      if (bibFilter === 'unused' && !e.unused) return false;
      if (bibFilter === 'review' && !needsReview(e.key)) return false;
      const q = bibQuery.trim().toLowerCase();
      return (
        !q ||
        e.key.toLowerCase().includes(q) ||
        tex(e.fields.title).toLowerCase().includes(q) ||
        tex(e.fields.author).toLowerCase().includes(q)
      );
    }),
  );
  let undefinedCites = $derived(app.check?.issues.filter((i) => i.code === 'undefined_cite') ?? []);
  // 大纲：折叠状态、可见行、当前章节跟随光标
  let folded = $state<Record<string, boolean>>({});
  let rows = $derived(app.outlineRows.filter((r) => !r.parents.some((p) => folded[p])));
  let sectionCount = $derived(app.outlineRows.length);
  let outlineEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    const id = app.currentSectionId;
    if (!id || !outlineEl) return;
    void tick().then(() => outlineEl?.querySelector(`[data-id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'nearest' }));
  });

  const fmt = (n: number) => n.toLocaleString();

  // 大纲右键菜单
  let menu = $state<{ x: number; y: number; id: string; file: string; line: number } | null>(null);
  function openMenu(e: MouseEvent, r: { id: string; file: string; line: number }) {
    e.preventDefault();
    menu = { x: e.clientX, y: e.clientY, id: r.id, file: r.file, line: r.line };
  }
  /** 先取出菜单对应的章节再关闭菜单（关闭后 menu 为 null） */
  function menuRun(f: (t: NonNullable<typeof menu>) => unknown) {
    const t = menu;
    menu = null;
    if (t) void f(t);
  }

  // 拖动章节调整顺序：只能放在同级章节之前或之后（用指针事件实现，避免与拖入图片冲突）
  let drag = $state<{ id: string; depth: number; parents: string; x: number; y: number; active: boolean } | null>(null);
  let drop = $state<{ id: string; position: 'before' | 'after' } | null>(null);
  let suppressClick = false;

  /** 插入线画在哪一行的哪条边：放在展开的章节之后时，画在它最后一个可见子节的下方 */
  let indicator = $derived.by(() => {
    if (!drop) return null;
    if (drop.position === 'before') return { id: drop.id, edge: 'top' };
    const last = rows.filter((r) => r.parents.includes(drop!.id)).at(-1);
    return { id: last?.id ?? drop.id, edge: 'bottom' };
  });

  function dragStart(e: PointerEvent, r: { id: string; depth: number; parents: string[] }) {
    if (e.button !== 0 || app.moving) return;
    drag = { id: r.id, depth: r.depth, parents: r.parents.join('/'), x: e.clientX, y: e.clientY, active: false };
    window.addEventListener('pointermove', dragMove);
    window.addEventListener('pointerup', dragEnd);
    window.addEventListener('keydown', dragKey);
  }

  function dragMove(e: PointerEvent) {
    if (!drag) return;
    if (!drag.active) {
      if (Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 5) return;
      drag.active = true;
    }
    const el = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('.orow');
    const row = rows.find((r) => r.id === el?.dataset.id);
    if (!el || !row || row.id === drag.id || row.depth !== drag.depth) {
      drop = null;
      return;
    }
    const rect = el.getBoundingClientRect();
    drop = { id: row.id, position: e.clientY < rect.top + rect.height / 2 ? 'before' : 'after' };
  }

  function dragStop() {
    window.removeEventListener('pointermove', dragMove);
    window.removeEventListener('pointerup', dragEnd);
    window.removeEventListener('keydown', dragKey);
    drag = null;
    drop = null;
  }

  function dragEnd() {
    if (drag?.active) {
      suppressClick = true;
      setTimeout(() => (suppressClick = false));
      if (drop) void app.moveSection(drag.id, drop.id, drop.position);
    }
    dragStop();
  }

  function dragKey(e: KeyboardEvent) {
    if (e.key === 'Escape') dragStop();
  }

  // 侧栏宽度拖拽
  function startResize(e: PointerEvent) {
    const startX = e.clientX;
    const startW = app.layout.sidebarWidth;
    const move = (ev: PointerEvent) => {
      app.layout.sidebarWidth = Math.min(520, Math.max(200, startW + ev.clientX - startX));
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      app.saveLayout();
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }
</script>

{#snippet tree(nodes: TreeNode[], depth: number)}
  {#each nodes as n (n.path)}
    {@const b = badge(n.path)}
    <button class="row" class:on={app.active === n.path} style:padding-left="{8 + depth * 14}px" onclick={() => openNode(n)}>
      {#if n.kind === 'dir'}
        <span class="twist"><Icon name={expanded[n.path] ? 'chevronDown' : 'chevron'} size={14} /></span>
      {:else}
        <span class="ext ext-{n.kind}">{n.kind === 'tex' ? 'TeX' : n.kind === 'bib' ? 'bib' : n.kind === 'image' ? 'img' : ''}</span>
      {/if}
      <span class="name">{n.name}</span>
      {#if n.path === app.project?.main}<span class="main-tag" title={tr('主文件', 'Main file')}>{tr('主', 'main')}</span>{/if}
      {#if app.isDirty(n.path)}<span class="dirty" title={tr('未保存', 'Unsaved')}>●</span>{/if}
      {#if b}<span class="badge" class:err={b.err}>{b.n}</span>{/if}
    </button>
    {#if n.children && expanded[n.path]}{@render tree(n.children, depth + 1)}{/if}
  {/each}
{/snippet}

<aside aria-label={tr('侧栏', 'Sidebar')} style:width="{app.layout.sidebarWidth}px">
  {#if app.activity === 'files'}
    <div class="sec-h">{tr('资源管理器', 'Explorer')} <span class="proj">{app.project?.name}</span></div>
    {@render tree(app.tree, 0)}
  {:else if app.activity === 'outline'}
    <div class="sec-h">{tr('论文大纲', 'Outline')}</div>
    {#if (app.project?.roots.length ?? 0) > 1}
      <div class="main-pick">
        <label for="mainsel" class="muted">{tr('主文件', 'Main file')}</label>
        <select id="mainsel" value={app.project?.main} onchange={(e) => app.setMain((e.currentTarget as HTMLSelectElement).value)}>
          {#each app.project?.roots ?? [] as r (r)}<option value={r}>{r}</option>{/each}
        </select>
      </div>
    {/if}
    {#if app.index}
      <div class="summary muted">
        {tr(`${sectionCount} 节 · 共 ${fmt(app.index.total_words)} ${app.wordUnit}`, `${sectionCount} sections · ${fmt(app.index.total_words)} ${app.wordUnit}`)}
      </div>
      <div class="outline" bind:this={outlineEl}>
        {#each rows as r (r.id)}
          <div
            class="orow"
            class:top={r.depth === 0}
            class:current={app.currentSectionId === r.id}
            class:dragging={drag?.active && drag.id === r.id}
            class:drop-top={indicator?.id === r.id && indicator.edge === 'top'}
            class:drop-bottom={indicator?.id === r.id && indicator.edge === 'bottom'}
            data-id={r.id}
            style:padding-left="{6 + r.depth * 14}px"
          >
            {#if r.hasChildren}
              <button class="fold" aria-label={folded[r.id] ? tr('展开', 'Expand') : tr('折叠', 'Collapse')} onclick={() => (folded[r.id] = !folded[r.id])}>
                <Icon name={folded[r.id] ? 'chevron' : 'chevronDown'} size={12} />
              </button>
            {:else}
              <span class="fold"></span>
            {/if}
            <button
              class="otitle"
              title="{r.number} {r.title} · {fmt(r.total)} {app.wordUnit}{tr(' · 拖动可调整顺序', ' · drag to reorder')}"
              onpointerdown={(e) => dragStart(e, r)}
              oncontextmenu={(e) => openMenu(e, r)}
              onclick={() => !suppressClick && app.openFile(r.file, r.line)}
            >
              {#if r.number}<span class="num">{r.number}</span>{/if}
              <span class="name">{r.title}</span>
              {#if r.depth === 0 && r.total > 0}<span class="count">{fmt(r.total)}</span>{/if}
            </button>
          </div>
        {/each}
      </div>
      {#if !app.index.outline.length}<p class="empty">{tr('还没有章节。写下 \\section{…} 后会出现在这里。', 'No sections yet. They appear here once you write \\section{…}.')}</p>{/if}
    {:else}
      <p class="empty">
        {tr('没有找到主文件（含 \\documentclass 的 .tex），无法生成大纲。', 'No main file (a .tex with \\documentclass) was found, so there is no outline.')}
      </p>
    {/if}
  {:else if app.activity === 'assets'}
    <div class="sec-h">{tr('图表资产', 'Figures & Tables')}</div>
    {#each groups as g (g.kind)}
      <button class="ghead" onclick={() => (assetFolded[g.kind] = !assetFolded[g.kind])}>
        <Icon name={assetFolded[g.kind] ? 'chevron' : 'chevronDown'} size={12} />
        <span>{kindHead[g.kind]}</span>
        <span class="count">{g.items.length}</span>
      </button>
      {#if !assetFolded[g.kind]}
        {#each g.items as a (a.span.file + a.span.start)}
          <button class="asset" onclick={() => app.openFile(a.span.file, a.span.line)}>
            {#if g.kind === 'figure'}
              <Thumb path={a.paths?.find((p) => p) ?? null} fallback={a.files.length ? '…' : kindName.figure} />
            {/if}
            <span class="meta">
              <span class="line1">
                {#if a.no}<span class="no">{a.no}</span>{/if}
                <span class="cap" title={a.caption ?? ''}>{a.caption ?? a.label ?? kindName[g.kind]}</span>
              </span>
              <span class="line2">
                {#if a.label}<span class="mono label">{a.label}</span>{/if}
                {#if a.references.length}
                  <span class="muted">{tr(`引用 ${a.references.length} 次`, `cited ${a.references.length}×`)}</span>
                {:else if g.kind !== 'equation'}
                  <span class="c-warn">{tr('未被引用', 'not referenced')}</span>
                {/if}
                {#if a.paths?.some((p) => p === null)}<span class="c-err">{tr('图片缺失', 'image missing')}</span>{/if}
              </span>
            </span>
          </button>
        {/each}
      {/if}
    {:else}
      <p class="empty">{tr('还没有 figure / table / 带 label 的公式。', 'No figures, tables or labeled equations yet.')}</p>
    {/each}
  {:else}
    <div class="sec-h bib-h">
      {tr('文献库', 'Bibliography')}
      <button
        class="sec-act"
        disabled={app.verifyingBib || !app.check?.bib.length}
        title={tr('到 Crossref / arXiv 核对每条文献是否存在、信息是否一致（会发送标题、作者、年份、DOI）', 'Check every reference against Crossref / arXiv (sends titles, authors, years and DOIs)')}
        onclick={() => void app.verifyBib()}>{app.verifyingBib ? tr('核查中…', 'Checking…') : tr('核查', 'Verify')}</button
      >
    </div>
    <form class="import" onsubmit={(e) => { e.preventDefault(); void doImport(); }}>
      <label class="sr-only" for="bibimport">{tr('通过 DOI 或 arXiv 导入', 'Import by DOI or arXiv')}</label>
      <input id="bibimport" placeholder={tr('粘贴 DOI、arXiv 编号或链接', 'Paste a DOI, arXiv ID or link')} bind:value={importId} disabled={app.importingBib} />
      <button class="btn btn-pri" type="submit" disabled={!importId.trim() || app.importingBib}>{app.importingBib ? tr('导入中…', 'Importing…') : tr('导入', 'Import')}</button>
    </form>
    <div class="search">
      <label class="sr-only" for="bibq">{tr('搜索文献', 'Search references')}</label>
      <input id="bibq" placeholder={tr('搜索 key、标题或作者', 'Search key, title or author')} bind:value={bibQuery} />
      <div class="chips" role="group" aria-label={tr('筛选', 'Filter')}>
        <button class="chip" class:on={bibFilter === 'all'} onclick={() => (bibFilter = 'all')}>{tr('全部', 'All')} {app.check?.bib.length ?? 0}</button>
        <button class="chip" class:on={bibFilter === 'unused'} onclick={() => (bibFilter = 'unused')} disabled={!unusedCount}>{tr('未使用', 'Unused')} {unusedCount}</button>
        {#if verified}
          <button class="chip" class:on={bibFilter === 'review'} onclick={() => (bibFilter = 'review')} disabled={!reviewCount}
            >{tr('需核对', 'Review')} {reviewCount}</button
          >
        {/if}
      </div>
    </div>
    {#each undefinedCites as u (u.key + String(u.line))}
      <button class="bibr" onclick={() => u.file && app.openFile(u.file, u.line ?? 1)}>
        <span class="bk"><span class="mono key">{u.key}</span><span class="c-err tag">{tr('未定义', 'undefined')}</span></span>
        <span class="muted small">{tr(`${u.file}:${u.line} 引用了它，但文献库中没有`, `cited at ${u.file}:${u.line} but not in the bibliography`)}</span>
      </button>
    {/each}
    {#each bib as e (e.span.file + e.key + e.span.start)}
      {@const v = app.bibVerify[e.key]}
      <div class="bibw">
      <button class="bibr" onclick={() => app.openFile(e.span.file, e.span.line)}>
        <span class="bk">
          <span class="mono key">{e.key}</span>
          {#if v}<span class="vmark {vmark[v.status].cls}" title={v.notes.join('\n') || tr('已核对，信息一致', 'Checked; details match')}>{vmark[v.status].icon}</span>{/if}
          {#if e.citations.length}
            <span class="tag muted">{tr(`引用 ${e.citations.length}`, `cited ${e.citations.length}`)}</span>
          {:else if e.unused}
            <span class="tag c-warn">{tr('未使用', 'unused')}</span>
          {:else}
            <span
              class="tag muted"
              title={tr('正文未引用，但同目录的其他文档（如补充材料）引用了它', 'Not cited in the paper, but cited by another document in the folder (such as supplementary material)')}
              >{tr('其他文档引用', 'cited elsewhere')}</span
            >
          {/if}
        </span>
        <span class="title">{tex(e.fields.title) || tr('（无标题）', '(untitled)')}</span>
        <span class="muted small">{[authors(e.fields.author), e.fields.year].filter(Boolean).join(' · ')}{e.fields.journal || e.fields.booktitle ? ` · ${tex(e.fields.journal ?? e.fields.booktitle)}` : ''}</span>
        {#if v && v.status !== 'ok'}{#each v.notes as n (n)}<span class="small vnote {vmark[v.status].cls}">{n}</span>{/each}{/if}
      </button>
      {#if v?.suggested_doi}
        <button class="add-doi" onclick={() => void app.addDoi(e.key, v.suggested_doi!)}>{tr(`补充 DOI：${v.suggested_doi}`, `Add DOI: ${v.suggested_doi}`)}</button>
      {/if}
      </div>
    {:else}
      <p class="empty">{bibQuery ? tr('没有匹配的文献', 'No matching references') : tr('项目没有引用 .bib 文件', 'The project does not use a .bib file')}</p>
    {/each}
  {/if}
</aside>
{#if menu}
  <!-- 点击菜单外或按 Esc 关闭 -->
  <div class="menu-mask" role="presentation" onpointerdown={() => (menu = null)} oncontextmenu={(e) => { e.preventDefault(); menu = null; }}></div>
  <div class="menu" role="menu" tabindex="-1" style:left="{menu.x}px" style:top="{menu.y}px" onkeydown={(e) => e.key === 'Escape' && (menu = null)}>
    <button role="menuitem" onclick={() => menuRun((t) => app.openFile(t.file, t.line))}>{tr('打开', 'Open')}</button>
    <button role="menuitem" onclick={() => menuRun((t) => app.forwardSync(t.file, t.line))}>{tr('在 PDF 中定位', 'Show in PDF')}</button>
    <button role="menuitem" onclick={() => menuRun((t) => app.extractSection(t.id))}>{tr('拆成独立文件', 'Move to Its Own File')}</button>
  </div>
{/if}

<div class="resizer" role="separator" aria-orientation="vertical" aria-label={tr('调整侧栏宽度', 'Resize sidebar')} onpointerdown={startResize}></div>

<style>
  aside {
    flex: 0 0 auto;
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
  .resizer {
    flex: 0 0 4px;
    margin-left: -4px;
    cursor: col-resize;
    position: relative;
    z-index: 5;
  }
  .resizer:hover,
  .resizer:active {
    background: var(--accent);
    opacity: 0.4;
  }
  .summary {
    padding: 0 12px 8px;
    font-size: 12px;
  }
  .orow {
    display: flex;
    align-items: center;
    min-height: 24px;
    padding-right: 10px;
  }
  .orow:hover {
    background: var(--hover);
  }
  .orow.current {
    background: var(--active);
  }
  .menu-mask {
    position: fixed;
    inset: 0;
    z-index: 30;
  }
  .menu {
    position: fixed;
    z-index: 31;
    min-width: 168px;
    padding: 4px;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.14);
    display: flex;
    flex-direction: column;
  }
  .menu button {
    text-align: left;
    padding: 5px 10px;
    border-radius: 5px;
    font-size: 13px;
  }
  .menu button:hover {
    background: var(--hover);
  }
  .orow.dragging {
    opacity: 0.45;
  }
  .orow.drop-top {
    box-shadow: inset 0 2px 0 var(--accent);
  }
  .orow.drop-bottom {
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .outline {
    user-select: none;
  }
  .fold {
    flex: 0 0 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg3);
    border-radius: 3px;
  }
  button.fold:hover {
    color: var(--fg);
  }
  .otitle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 3px 0;
  }
  .otitle .name {
    flex: 1 1 auto;
    min-width: 0;
    text-align: left;
  }
  .orow.top .otitle {
    font-weight: 500;
  }
  .num {
    flex: 0 0 auto;
    color: var(--fg3);
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  .count {
    flex: 0 0 auto;
    margin-left: auto;
    padding-left: 8px;
    font-size: 11px;
    font-weight: 400;
    color: var(--fg3);
    font-variant-numeric: tabular-nums;
  }
  .main-pick {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px 10px;
    font-size: 12px;
  }
  .main-pick select {
    flex: 1;
    min-width: 0;
    height: 26px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--fg);
    font: inherit;
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
  .meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
    font-size: 12px;
  }
  .line1 {
    display: flex;
    gap: 6px;
    min-width: 0;
  }
  .no {
    flex: 0 0 auto;
    color: var(--fg2);
    font-weight: 500;
  }
  .cap {
    min-width: 0;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    line-height: 1.4;
  }
  .line2 {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 8px;
    font-size: 11px;
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 12px 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--fg2);
  }
  .ghead .count {
    margin-left: 0;
    padding-left: 0;
  }
  .chips {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }
  .chips .chip {
    cursor: pointer;
    padding: 2px 9px;
  }
  .chips .chip.on {
    background: var(--active);
    color: var(--fg);
    border-color: var(--fg3);
  }
  .chips .chip[disabled] {
    opacity: 0.5;
    cursor: default;
  }
  .proj {
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
    margin-left: 4px;
  }
  .main-tag {
    flex: 0 0 auto;
    font-size: 10px;
    padding: 0 4px;
    border-radius: 3px;
    border: 1px solid var(--line);
    color: var(--fg2);
  }
  .label {
    color: var(--tk-label);
    font-size: 11px;
  }
  .search {
    padding: 0 12px 8px;
  }
  .import {
    display: flex;
    gap: 6px;
    padding: 0 12px 10px;
  }
  .import input {
    flex: 1;
    min-width: 0;
    height: 28px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--bg);
    padding: 0 8px;
    font-size: 12px;
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
  .bib-h {
    display: flex;
    align-items: center;
  }
  .sec-act {
    margin-left: auto;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0;
    text-transform: none;
    color: var(--accent);
    padding: 1px 6px;
    border-radius: 4px;
  }
  .sec-act:hover:not(:disabled) {
    background: var(--hover);
  }
  .sec-act:disabled {
    color: var(--fg3);
  }
  .vmark {
    font-size: 12px;
    font-weight: 600;
  }
  .vnote {
    line-height: 1.4;
  }
  .add-doi {
    margin: -4px 12px 8px;
    font-size: 12px;
    color: var(--accent);
    text-align: left;
    overflow-wrap: anywhere;
  }
  .add-doi:hover {
    text-decoration: underline;
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
