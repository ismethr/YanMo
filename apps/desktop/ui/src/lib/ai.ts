// AI 调用。桌面端经 Rust（ym-ai）发出请求并以 `ai-event` 事件流式返回；
// 浏览器预览模式用模拟数据，便于只改界面的贡献者调试。

import { invoke } from '@tauri-apps/api/core';
import { inTauri } from './backend';

export type ProviderKind = 'anthropic' | 'openai_compatible';

export interface AiSettings {
  provider: ProviderKind;
  anthropic_model: string;
  effort: string;
  openai_base_url: string;
  openai_model: string;
}

export interface AiSettingsInfo {
  settings: AiSettings;
  has_key: Record<ProviderKind, boolean>;
}

export type RewriteAction = 'polish' | 'academic' | 'translate_to_english' | 'translate_to_chinese' | 'shorten' | 'expand';

export type AiTask =
  | { kind: 'rewrite'; action: RewriteAction; selection: string }
  | { kind: 'chat'; history: { role: 'user' | 'assistant'; content: string }[] }
  | { kind: 'explain_error'; diagnostics: string; source: string };

export interface AiContext {
  file?: string | null;
  section_title?: string | null;
  section_text?: string | null;
  outline?: string | null;
}

export interface AiResult {
  outcome: { text: string; model: string; stop_reason: string | null; input_tokens: number; output_tokens: number };
  rewrite: string | null;
}

type StreamEvent = { type: 'text'; value: string } | { type: 'notice'; value: string };

let previewSettings: AiSettings = {
  provider: 'anthropic',
  anthropic_model: 'claude-opus-5-5',
  effort: 'medium',
  openai_base_url: 'http://localhost:11434/v1',
  openai_model: '',
};

export async function getAiSettings(): Promise<AiSettingsInfo> {
  if (inTauri) return invoke<AiSettingsInfo>('ai_settings_get');
  return { settings: previewSettings, has_key: { anthropic: true, openai_compatible: false } };
}

export async function saveAiSettings(settings: AiSettings): Promise<void> {
  if (inTauri) return invoke('ai_settings_set', { settings });
  previewSettings = settings;
}

/** 把 Key 存入系统钥匙串；空字符串表示删除。Key 不会再返回给界面。 */
export async function setAiKey(provider: ProviderKind, key: string): Promise<void> {
  if (inTauri) return invoke('ai_set_key', { provider, key });
}

let seq = 0;

export function cancelAi(id: string) {
  if (inTauri) void invoke('ai_cancel', { id });
  previewCancelled.add(id);
}

const previewCancelled = new Set<string>();

/** 执行一次 AI 任务；增量文本经 onText 回调返回。返回 id 便于取消。 */
export function runAi(
  task: AiTask,
  context: AiContext,
  onText: (t: string) => void,
  onNotice: (n: string) => void,
): { id: string; done: Promise<AiResult> } {
  const id = `ai-${Date.now()}-${++seq}`;
  if (!inTauri) return { id, done: preview(id, task, onText) };
  const done = (async () => {
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ id: string; event: StreamEvent }>('ai-event', (e) => {
      if (e.payload.id !== id) return;
      if (e.payload.event.type === 'text') onText(e.payload.event.value);
      else onNotice(e.payload.event.value);
    });
    try {
      return await invoke<AiResult>('ai_stream', { id, task, context });
    } finally {
      unlisten();
    }
  })();
  return { id, done };
}

async function preview(id: string, task: AiTask, onText: (t: string) => void): Promise<AiResult> {
  let text: string;
  let rewrite: string | null = null;
  if (task.kind === 'rewrite') {
    rewrite = task.selection.replace('改变了序列建模', '从根本上改变了序列建模的范式').replace(/\s+/g, ' ').trim();
    text = '```latex\n' + rewrite + '\n```';
  } else if (task.kind === 'explain_error') {
    text = '（预览模式示例）`figures/arch.pdf` 不存在，所以 `\\includegraphics` 无法载入图片。把图片导出为 PDF 放到 figures/ 目录即可。';
  } else {
    text = '（预览模式示例回复）桌面应用中，这里会显示所配置模型的真实回答，并逐字流式显示。';
  }
  for (const ch of text.match(/.{1,6}/gs) ?? []) {
    if (previewCancelled.has(id)) throw new Error('已取消');
    onText(ch);
    await new Promise((r) => setTimeout(r, 25));
  }
  return { outcome: { text, model: previewSettings.anthropic_model, stop_reason: 'end_turn', input_tokens: 0, output_tokens: 0 }, rewrite };
}
