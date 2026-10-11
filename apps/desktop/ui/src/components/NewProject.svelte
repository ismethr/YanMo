<script lang="ts">
  // 从模板新建论文：选模板、填标题与文件夹名、选位置，创建后直接打开。
  import { onMount } from 'svelte';
  import { app } from '../lib/store.svelte';
  import { execute, inTauri } from '../lib/backend';
  import { i18n, tr } from '../lib/i18n.svelte';

  interface Template {
    id: string;
    name: string;
    name_en: string;
    description: string;
    description_en: string;
  }

  let templates = $state<Template[]>([]);
  let template = $state('');
  let title = $state('');
  let name = $state('');
  let nameTouched = $state(false);
  let parent = $state('');
  let creating = $state(false);
  let error = $state<string | null>(null);
  let titleInput: HTMLInputElement | undefined = $state();

  onMount(async () => {
    titleInput?.focus();
    try {
      templates = await execute<Template[]>('project.templates');
      template = i18n.lang === 'zh' ? 'article-zh' : 'article-en';
    } catch (e) {
      error = String(e);
    }
    if (inTauri) {
      const { documentDir } = await import('@tauri-apps/api/path');
      parent = await documentDir().catch(() => '');
    }
  });

  // 文件夹名默认取自标题：保留字母、数字、汉字，其余换成 -
  $effect(() => {
    if (nameTouched) return;
    name = title
      .trim()
      .replace(/[^\p{L}\p{N}]+/gu, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 48);
  });

  async function pickParent() {
    if (!inTauri) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({ directory: true, multiple: false, title: tr('选择新论文的位置', 'Choose where to create the paper') });
    if (typeof picked === 'string') parent = picked;
  }

  async function create() {
    error = null;
    if (!name.trim() || !parent) return;
    creating = true;
    try {
      const { path } = await execute<{ path: string }>('project.create', { template, parent, name: name.trim(), title: title.trim() });
      app.newProjectOpen = false;
      await app.openFolder(path);
      await app.openFile('main.tex');
    } catch (e) {
      error = String(e).replace(/^Error:\s*/, '');
    } finally {
      creating = false;
    }
  }

  function close() {
    app.newProjectOpen = false;
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<div class="backdrop" role="presentation" onclick={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={tr('从模板新建论文', 'New Paper from Template')} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
    <h2>{tr('从模板新建论文', 'New Paper from Template')}</h2>

    <div class="grid" role="radiogroup" aria-label={tr('模板', 'Template')}>
      {#each templates as t (t.id)}
        <button class="tpl" class:on={template === t.id} role="radio" aria-checked={template === t.id} onclick={() => (template = t.id)}>
          <span class="name">{i18n.lang === 'zh' ? t.name : t.name_en}</span>
          <span class="muted small">{i18n.lang === 'zh' ? t.description : t.description_en}</span>
        </button>
      {/each}
    </div>

    <label class="field">
      <span>{tr('论文标题', 'Title')}</span>
      <input bind:this={titleInput} bind:value={title} placeholder={tr('可以之后再改', 'You can change it later')} />
    </label>
    <label class="field">
      <span>{tr('文件夹名称', 'Folder name')}</span>
      <input bind:value={name} oninput={() => (nameTouched = true)} placeholder="my-paper" />
    </label>
    <div class="field">
      <span>{tr('位置', 'Location')}</span>
      <div class="where">
        <span class="mono path" title={parent}>{parent ? `${parent}/${name || '…'}` : tr('未选择', 'Not chosen')}</span>
        <button class="btn" onclick={pickParent} disabled={!inTauri}>{tr('选择…', 'Choose…')}</button>
      </div>
    </div>

    {#if error}<p class="c-err small">{error}</p>{/if}

    <div class="actions">
      <button class="btn" onclick={close}>{tr('取消', 'Cancel')}</button>
      <button class="btn btn-pri" disabled={creating || !template || !name.trim() || !parent} onclick={create}
        >{creating ? tr('创建中…', 'Creating…') : tr('创建并打开', 'Create and Open')}</button
      >
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
    width: min(620px, 94vw);
    max-height: 90vh;
    overflow-y: auto;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.22);
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .tpl {
    display: flex;
    flex-direction: column;
    gap: 3px;
    text-align: left;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .tpl:hover {
    background: var(--hover);
  }
  .tpl.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .name {
    font-weight: 500;
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--fg2);
  }
  .field input {
    height: 30px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--fg);
    padding: 0 10px;
    font: inherit;
    font-size: 13px;
  }
  .where {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
