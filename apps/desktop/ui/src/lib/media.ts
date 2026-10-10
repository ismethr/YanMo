// 图片与 PDF 图的预览：位图直接用 data URL，PDF 用 pdf.js 渲染第一页。结果按路径与尺寸缓存。

import * as pdfjs from 'pdfjs-dist';
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
import { execute } from './backend';

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

const cache = new Map<string, Promise<string>>();

export const IMAGE_EXT = /\.(png|jpe?g|gif|svg|pdf)$/i;

function toBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

async function renderPdf(data: Uint8Array, width: number): Promise<string> {
  const task = pdfjs.getDocument({ data });
  try {
    const doc = await task.promise;
    const page = await doc.getPage(1);
    const base = page.getViewport({ scale: 1 });
    const ratio = window.devicePixelRatio || 1;
    const vp = page.getViewport({ scale: (width / base.width) * ratio });
    const canvas = document.createElement('canvas');
    canvas.width = Math.ceil(vp.width);
    canvas.height = Math.ceil(vp.height);
    await page.render({ canvas, viewport: vp }).promise;
    return canvas.toDataURL('image/png');
  } finally {
    void task.destroy();
  }
}

/** 返回可直接用于 <img src> 的地址；`width` 为显示宽度（CSS 像素），仅影响 PDF 的渲染清晰度。 */
export function previewUrl(path: string, width: number): Promise<string> {
  const key = `${path}@${Math.round(width)}`;
  let p = cache.get(key);
  if (!p) {
    p = execute<{ mime: string; base64: string }>('file.base64', { path }).then((f) =>
      f.mime === 'application/pdf' ? renderPdf(toBytes(f.base64), width) : `data:${f.mime};base64,${f.base64}`,
    );
    p.catch(() => cache.delete(key));
    cache.set(key, p);
  }
  return p;
}

/** 文件更新后（如重新导出图片）清除缓存 */
export function invalidatePreview(path?: string) {
  for (const k of [...cache.keys()]) if (!path || k.startsWith(`${path}@`)) cache.delete(k);
}
