<script lang="ts">
  // PDF 预览：pdf.js 逐页渲染到 canvas；点击页面反向跳回源码，正向同步时高亮目标区域。
  import { onMount, tick } from 'svelte';
  import * as pdfjs from 'pdfjs-dist';
  import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
  import { app } from '../lib/store.svelte';

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

  interface PageView {
    n: number;
    width: number;
    height: number;
    canvas: HTMLCanvasElement;
  }

  let scroller: HTMLDivElement;
  let pages = $state<PageView[]>([]);
  let scale = $state(1);
  let zoom = $state(1);
  let fitWidth = $state(1);
  let highlight = $state<{ page: number; left: number; top: number; width: number; height: number; n: number } | null>(null);
  let task: pdfjs.PDFDocumentLoadingTask | null = null;
  let renderSeq = 0;

  async function render() {
    // 标签页隐藏时宽度为 0，等重新显示时由 ResizeObserver 触发渲染
    if (!app.pdf || !scroller || scroller.clientWidth === 0) return;
    const seq = ++renderSeq;
    // pdf.js 会转移传入的缓冲区，传副本
    const loading = pdfjs.getDocument({ data: app.pdf.data.slice() });
    const loaded = await loading.promise;
    if (seq !== renderSeq) return void loading.destroy();
    const first = await loaded.getPage(1);
    const base = first.getViewport({ scale: 1 });
    fitWidth = Math.max(0.4, (scroller.clientWidth - 48) / base.width);
    const s = Math.min(4, fitWidth * zoom);
    const dpr = window.devicePixelRatio || 1;

    const views: PageView[] = [];
    for (let n = 1; n <= loaded.numPages; n++) {
      const page = await loaded.getPage(n);
      const vp = page.getViewport({ scale: s });
      const canvas = document.createElement('canvas');
      canvas.width = Math.floor(vp.width * dpr);
      canvas.height = Math.floor(vp.height * dpr);
      canvas.style.width = `${vp.width}px`;
      canvas.style.height = `${vp.height}px`;
      await page.render({ canvas, viewport: vp, transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined }).promise;
      if (seq !== renderSeq) return void loading.destroy();
      views.push({ n, width: vp.width, height: vp.height, canvas });
    }
    // 新版本渲染完成后整体替换，并按比例保持滚动位置，避免闪烁和跳动
    const ratio = scroller.scrollHeight ? scroller.scrollTop / scroller.scrollHeight : 0;
    void task?.destroy();
    task = loading;
    scale = s;
    pages = views;
    await tick();
    scroller.scrollTop = ratio * scroller.scrollHeight;
  }

  function mount(node: HTMLDivElement, canvas: HTMLCanvasElement) {
    node.replaceChildren(canvas);
    return {
      update(c: HTMLCanvasElement) {
        node.replaceChildren(c);
      },
    };
  }

  function onPageClick(e: MouseEvent, p: PageView) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    void app.inverseSync(p.n, (e.clientX - rect.left) / scale, (e.clientY - rect.top) / scale);
  }

  function setZoom(z: number) {
    zoom = Math.min(3, Math.max(0.5, z));
    void render();
  }

  // 新 PDF
  $effect(() => {
    if (app.pdf?.rev) void render();
  });

  // 正向同步：滚动到目标并高亮
  $effect(() => {
    const t = app.syncTarget;
    if (!t || !pages.length) return;
    highlight = { page: t.page, left: t.x * scale, top: t.y * scale, width: t.w * scale, height: t.h * scale, n: t.n };
    void tick().then(() => {
      const el = scroller.querySelector<HTMLElement>(`[data-page="${t.page}"]`);
      if (el) scroller.scrollTo({ top: el.offsetTop + t.y * scale - scroller.clientHeight / 3, behavior: 'smooth' });
    });
  });

  onMount(() => {
    let timer: ReturnType<typeof setTimeout>;
    const ro = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => void render(), 150);
    });
    ro.observe(scroller);
    return () => {
      ro.disconnect();
      void task?.destroy();
    };
  });
</script>

<div class="viewer">
  <div class="toolbar">
    <span>{pages.length ? `${pages.length} 页` : '加载中…'}</span>
    <span class="grow"></span>
    <button aria-label="缩小" onclick={() => setZoom(zoom - 0.1)}>−</button>
    <span class="pct">{Math.round(zoom * 100)}%</span>
    <button aria-label="放大" onclick={() => setZoom(zoom + 0.1)}>+</button>
    <button onclick={() => setZoom(1)}>适合宽度</button>
    <span class="hint">点击页面跳回源码</span>
  </div>
  <div class="scroller" bind:this={scroller}>
    {#each pages as p (p.n)}
      <div class="page" data-page={p.n} style:width="{p.width}px" style:height="{p.height}px">
        <div class="canvas" use:mount={p.canvas}></div>
        <button class="hit" aria-label="第 {p.n} 页，点击跳到对应源码" onclick={(e) => onPageClick(e, p)}></button>
        {#if highlight && highlight.page === p.n}
          {#key highlight.n}
            <div
              class="mark"
              style:left="{highlight.left - 3}px"
              style:top="{highlight.top - 2}px"
              style:width="{highlight.width + 6}px"
              style:height="{highlight.height + 4}px"
            ></div>
          {/key}
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .viewer {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .toolbar {
    height: 24px;
    flex: 0 0 24px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 14px;
    font-size: 12px;
    color: var(--fg2);
    border-bottom: 1px solid var(--line);
  }
  .toolbar button {
    padding: 0 6px;
    border-radius: 4px;
  }
  .toolbar button:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .grow {
    flex: 1;
  }
  .pct {
    min-width: 36px;
    text-align: center;
  }
  .hint {
    margin-left: 10px;
  }
  .scroller {
    flex: 1;
    overflow: auto;
    background: var(--pdfbg);
    padding: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }
  .page {
    position: relative;
    flex: 0 0 auto;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12), 0 6px 24px rgba(0, 0, 0, 0.1);
  }
  .canvas,
  .hit {
    position: absolute;
    inset: 0;
  }
  .hit {
    cursor: pointer;
  }
  .mark {
    position: absolute;
    pointer-events: none;
    border-radius: 3px;
    background: rgba(255, 196, 0, 0.32);
    outline: 1.5px solid rgba(214, 150, 0, 0.7);
    animation: fade 2.4s ease-out forwards;
  }
  @keyframes fade {
    0%,
    60% {
      opacity: 1;
    }
    100% {
      opacity: 0;
    }
  }
</style>
