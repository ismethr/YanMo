<script lang="ts">
  // 缩略图：滚动到可见时才加载。
  import { onMount } from 'svelte';
  import { previewUrl } from '../lib/media';

  let { path, width = 64, height = 48, fallback = '' }: { path: string | null; width?: number; height?: number; fallback?: string } =
    $props();

  let el: HTMLDivElement;
  let src = $state<string | null>(null);
  let failed = $state(false);

  onMount(() => {
    if (!path) return;
    const io = new IntersectionObserver((entries) => {
      if (!entries.some((e) => e.isIntersecting) || !path) return;
      io.disconnect();
      previewUrl(path, width * 2).then(
        (u) => (src = u),
        () => (failed = true),
      );
    });
    io.observe(el);
    return () => io.disconnect();
  });
</script>

<div class="thumb" bind:this={el} style:width="{width}px" style:height="{height}px">
  {#if src}
    <img {src} alt="" />
  {:else}
    <span>{failed ? '无预览' : fallback}</span>
  {/if}
</div>

<style>
  .thumb {
    flex: 0 0 auto;
    border-radius: 4px;
    border: 1px solid var(--line);
    background: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    color: var(--fg3);
    font-size: 11px;
  }
  img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
</style>
