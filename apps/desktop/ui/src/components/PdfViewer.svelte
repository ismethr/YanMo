<script lang="ts">
  // PDF 预览（pdf.js）。
  // - 虚拟化：只渲染视口附近的页面，远离的页面释放画布，长文档滚动也不卡；
  // - 缩放：适合宽度 / 适合整页 / 百分比，触控板双指缩放、⌘+ ⌘- ⌘0；缩放时先用 CSS 即时拉伸，停下后再按清晰度重绘；
  // - 页面始终居中，放大后可横向滚动到任一侧；
  // - 点击页面反向跳回源码，正向同步时高亮目标区域。
  import { onMount, tick } from 'svelte';
  import * as pdfjs from 'pdfjs-dist';
  import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
  import { app, PDF_TAB } from '../lib/store.svelte';

  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

  /** CSS 像素 / PDF 点：100% 对应 96/72 */
  const PX_PER_PT = 96 / 72;
  const PAD = 24;
  const GAP = 16;
  /** WebKit 单个画布的像素上限约 1678 万，超过会整块空白 */
  const MAX_CANVAS_PIXELS = 16_000_000;
  const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3];

  type Mode = 'width' | 'page' | 'custom';

  let scroller: HTMLDivElement;
  let sizes = $state<{ w: number; h: number }[]>([]);
  let mode = $state<Mode>('width');
  let customZoom = $state(1);
  let viewW = $state(0);
  let viewH = $state(0);
  let currentPage = $state(1);
  let highlight = $state<{ page: number; x: number; y: number; w: number; h: number; n: number } | null>(null);

  // 按出现最多的页面尺寸适配：论文里偶尔的横向页（宽表格）不应让其余页面整体缩小
  let typical = $derived.by(() => {
    const count = new Map<string, { w: number; h: number; n: number }>();
    for (const s of sizes) {
      const k = `${Math.round(s.w)}x${Math.round(s.h)}`;
      const c = count.get(k) ?? { ...s, n: 0 };
      c.n++;
      count.set(k, c);
    }
    return [...count.values()].sort((a, b) => b.n - a.n)[0] ?? { w: 1, h: 1, n: 0 };
  });
  let scale = $derived.by(() => {
    if (mode === 'custom') return customZoom * PX_PER_PT;
    if (!sizes.length || viewW === 0) return PX_PER_PT;
    const fitW = (viewW - 2 * PAD) / typical.w;
    if (mode === 'width') return Math.max(0.2, fitW);
    return Math.max(0.2, Math.min(fitW, (viewH - 2 * PAD) / typical.h));
  });
  let percent = $derived(Math.round((scale / PX_PER_PT) * 100));

  // ---- 文档与页面渲染 ----

  let task: pdfjs.PDFDocumentLoadingTask | null = null;
  let doc: pdfjs.PDFDocumentProxy | null = null;
  const slots: HTMLDivElement[] = [];
  /** 已渲染的页面：当前显示的画布、渲染时的缩放、进行中的渲染任务 */
  const rendered = new Map<number, { canvas: HTMLCanvasElement | null; scale: number; job?: pdfjs.RenderTask }>();
  const visible = new Set<number>();
  let observer: IntersectionObserver | null = null;
  let renderTimer: ReturnType<typeof setTimeout> | undefined;

  async function load() {
    if (!app.pdf) return;
    const loading = pdfjs.getDocument({ data: app.pdf.data.slice() });
    const loaded = await loading.promise;
    const dims: { w: number; h: number }[] = [];
    for (let n = 1; n <= loaded.numPages; n++) {
      const vp = (await loaded.getPage(n)).getViewport({ scale: 1 });
      dims.push({ w: vp.width, h: vp.height });
    }
    // 新版本就绪后再替换旧文档；旧画布在新画布绘好前继续显示，重新编译时不闪烁
    const old = task;
    task = loading;
    doc = loaded;
    for (const r of rendered.values()) {
      r.job?.cancel();
      r.scale = -1;
    }
    sizes = dims;
    measure();
    await tick();
    scheduleRender(0);
    void old?.destroy();
  }

  /** 读取容器尺寸。查看器可能在隐藏状态下挂载，不能只依赖 ResizeObserver 的回调 */
  function measure() {
    if (!scroller) return;
    if (scroller.clientWidth !== viewW) viewW = scroller.clientWidth;
    if (scroller.clientHeight !== viewH) viewH = scroller.clientHeight;
  }

  function scheduleRender(delay = 150) {
    clearTimeout(renderTimer);
    renderTimer = setTimeout(renderVisible, delay);
  }

  async function renderPage(n: number) {
    const slot = slots[n - 1];
    const d = doc;
    if (!d || !slot) return;
    const target = scale;
    const prev = rendered.get(n);
    if (prev && prev.scale === target) return;
    prev?.job?.cancel();
    const entry = { canvas: prev?.canvas ?? null, scale: target, job: undefined as pdfjs.RenderTask | undefined };
    rendered.set(n, entry);

    const page = await d.getPage(n);
    if (rendered.get(n) !== entry) return;
    const vp = page.getViewport({ scale: target });
    let ratio = window.devicePixelRatio || 1;
    if (vp.width * vp.height * ratio * ratio > MAX_CANVAS_PIXELS) {
      ratio = Math.sqrt(MAX_CANVAS_PIXELS / (vp.width * vp.height));
    }
    const canvas = document.createElement('canvas');
    canvas.width = Math.floor(vp.width * ratio);
    canvas.height = Math.floor(vp.height * ratio);
    const job = page.render({ canvas, viewport: vp, transform: ratio !== 1 ? [ratio, 0, 0, ratio, 0, 0] : undefined });
    entry.job = job;
    try {
      await job.promise;
    } catch {
      return; // 被取消
    }
    if (rendered.get(n) !== entry) return;
    entry.job = undefined;
    entry.canvas?.remove();
    entry.canvas = canvas;
    slot.prepend(canvas);
  }

  function renderVisible() {
    measure();
    // 先渲染离视口中心最近的页面
    const center = currentPage;
    const order = [...visible].sort((a, b) => Math.abs(a - center) - Math.abs(b - center));
    for (const n of order) void renderPage(n);
    for (const [n, r] of rendered) {
      if (!visible.has(n)) {
        r.job?.cancel();
        r.canvas?.remove();
        rendered.delete(n);
      }
    }
  }

  function slot(node: HTMLDivElement, n: number) {
    slots[n - 1] = node;
    observer?.observe(node);
    return {
      destroy() {
        observer?.unobserve(node);
        visible.delete(n);
      },
    };
  }

  // ---- 缩放（保持锚点下的内容不动） ----

  function setScale(apply: () => void, anchor?: { x: number; y: number }) {
    const ax = anchor?.x ?? scroller.clientWidth / 2;
    const ay = anchor?.y ?? scroller.clientHeight / 2;
    const fx = (scroller.scrollLeft + ax) / Math.max(1, scroller.scrollWidth);
    const fy = (scroller.scrollTop + ay) / Math.max(1, scroller.scrollHeight);
    apply();
    void tick().then(() => {
      scroller.scrollLeft = fx * scroller.scrollWidth - ax;
      scroller.scrollTop = fy * scroller.scrollHeight - ay;
    });
  }

  function zoomTo(z: number, anchor?: { x: number; y: number }) {
    setScale(() => {
      customZoom = Math.min(5, Math.max(0.25, z));
      mode = 'custom';
    }, anchor);
  }

  function step(dir: 1 | -1) {
    const cur = scale / PX_PER_PT;
    const next = dir > 0 ? ZOOMS.find((z) => z > cur + 0.01) : [...ZOOMS].reverse().find((z) => z < cur - 0.01);
    zoomTo(next ?? (dir > 0 ? cur * 1.25 : cur / 1.25));
  }

  function fit(m: 'width' | 'page') {
    setScale(() => (mode = m));
  }

  function onPick(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    if (v === 'width' || v === 'page') fit(v);
    else if (v) zoomTo(Number(v));
  }

  // 触控板双指缩放在 WebKit / Chromium 中表现为带 ctrlKey 的 wheel 事件
  function onWheel(e: WheelEvent) {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    const rect = scroller.getBoundingClientRect();
    const factor = Math.exp(-e.deltaY * 0.01);
    zoomTo((scale / PX_PER_PT) * factor, { x: e.clientX - rect.left, y: e.clientY - rect.top });
  }

  function onKey(e: KeyboardEvent) {
    if (app.active !== PDF_TAB || !(e.metaKey || e.ctrlKey)) return;
    if (e.key === '=' || e.key === '+') {
      e.preventDefault();
      step(1);
    } else if (e.key === '-') {
      e.preventDefault();
      step(-1);
    } else if (e.key === '0') {
      e.preventDefault();
      fit('width');
    }
  }

  // ---- 页码、点击、同步 ----

  // 当前页：视口上方三分之一处所在的页面（二分查找，滚动时开销很小）
  function onScroll() {
    const mid = scroller.scrollTop + scroller.clientHeight / 3;
    let lo = 0;
    let hi = slots.length - 1;
    while (lo < hi) {
      const m = (lo + hi + 1) >> 1;
      if (slots[m] && slots[m].offsetTop <= mid) lo = m;
      else hi = m - 1;
    }
    currentPage = lo + 1;
  }

  function onPageClick(e: MouseEvent, n: number) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    void app.inverseSync(n, (e.clientX - rect.left) / scale, (e.clientY - rect.top) / scale);
  }

  // 新 PDF
  $effect(() => {
    if (app.pdf?.rev) void load();
  });

  // 切换到 PDF 标签页时重新测量（隐藏期间尺寸为 0）
  $effect(() => {
    if (app.active === PDF_TAB) void tick().then(() => {
      measure();
      scheduleRender(0);
    });
  });

  // 缩放变化（含窗口尺寸变化导致的"适合宽度"变化）后，停下来再按清晰度重绘
  $effect(() => {
    void scale;
    scheduleRender();
  });

  // 正向同步：滚动到目标并高亮
  $effect(() => {
    const t = app.syncTarget;
    if (!t || !sizes.length) return;
    highlight = { page: t.page, x: t.x, y: t.y, w: t.w, h: t.h, n: t.n };
    void tick().then(() => {
      const el = slots[t.page - 1];
      if (!el) return;
      scroller.scrollTo({
        top: el.offsetTop + t.y * scale - scroller.clientHeight / 3,
        left: Math.max(0, el.offsetLeft + t.x * scale - scroller.clientWidth / 3),
        behavior: 'smooth',
      });
    });
  });

  onMount(() => {
    observer = new IntersectionObserver(
      (entries) => {
        for (const en of entries) {
          const n = slots.indexOf(en.target as HTMLDivElement) + 1;
          if (n < 1) continue;
          if (en.isIntersecting) visible.add(n);
          else visible.delete(n);
        }
        measure();
        scheduleRender(40);
      },
      { root: scroller, rootMargin: '600px 300px' },
    );
    for (const s of slots) if (s) observer.observe(s);
    const ro = new ResizeObserver(measure);
    ro.observe(scroller);
    scroller.addEventListener('wheel', onWheel, { passive: false });
    return () => {
      ro.disconnect();
      observer?.disconnect();
      scroller.removeEventListener('wheel', onWheel);
      clearTimeout(renderTimer);
      for (const r of rendered.values()) r.job?.cancel();
      void task?.destroy();
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="viewer">
  <div class="toolbar">
    <span>{sizes.length ? `第 ${currentPage} / ${sizes.length} 页` : '加载中…'}</span>
    <span class="grow"></span>
    <button aria-label="缩小" title="缩小（⌘-）" onclick={() => step(-1)}>−</button>
    <label class="sr-only" for="pdfzoom">缩放</label>
    <select id="pdfzoom" value={mode === 'custom' ? '' : mode} onchange={onPick}>
      <option value="" disabled hidden>{percent}%</option>
      <option value="width">适合宽度</option>
      <option value="page">适合整页</option>
      {#each ZOOMS as z (z)}<option value={String(z)}>{Math.round(z * 100)}%</option>{/each}
    </select>
    <button aria-label="放大" title="放大（⌘+）" onclick={() => step(1)}>+</button>
    <span class="pct">{percent}%</span>
    <span class="hint">点击页面跳回源码 · 双指捏合或 ⌘ 加滚轮缩放</span>
  </div>
  <div class="scroller" bind:this={scroller} onscroll={onScroll}>
    <div class="stack" style:padding="{PAD}px" style:gap="{GAP}px">
      {#each sizes as s, i (i)}
        <div class="page" use:slot={i + 1} style:width="{s.w * scale}px" style:height="{s.h * scale}px">
          <button class="hit" aria-label="第 {i + 1} 页，点击跳到对应源码" onclick={(e) => onPageClick(e, i + 1)}></button>
          {#if highlight && highlight.page === i + 1}
            {#key highlight.n}
              <div
                class="mark"
                style:left="{highlight.x * scale - 3}px"
                style:top="{highlight.y * scale - 2}px"
                style:width="{highlight.w * scale + 6}px"
                style:height="{highlight.h * scale + 4}px"
              ></div>
            {/key}
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .viewer {
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .toolbar {
    height: 28px;
    flex: 0 0 28px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 14px;
    font-size: 12px;
    color: var(--fg2);
    border-bottom: 1px solid var(--line);
  }
  .toolbar button {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    text-align: center;
    font-size: 15px;
  }
  .toolbar button:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .toolbar select {
    height: 22px;
    border-radius: 4px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--fg);
    font: inherit;
  }
  .grow {
    flex: 1;
  }
  .pct {
    min-width: 38px;
  }
  .hint {
    margin-left: 8px;
  }
  .scroller {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--pdfbg);
    overscroll-behavior: contain;
  }
  /* 窄于视口时居中；宽于视口时撑开，左右两侧都能滚动到 */
  /* position: relative 让页面的 offsetTop/offsetLeft 以滚动内容为基准，与 scrollTop 一致 */
  .stack {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    width: max-content;
    min-width: 100%;
    box-sizing: border-box;
  }
  .page {
    position: relative;
    flex: 0 0 auto;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12), 0 4px 16px rgba(0, 0, 0, 0.08);
    contain: strict;
  }
  .page :global(canvas) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .hit {
    position: absolute;
    inset: 0;
    cursor: pointer;
    z-index: 1;
  }
  .mark {
    position: absolute;
    z-index: 2;
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
