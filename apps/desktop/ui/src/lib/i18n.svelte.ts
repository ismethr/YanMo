// 界面语言。文字直接以中英两份写在用到的地方：tr('打开文件夹', 'Open Folder')。
// tr() 读取响应式的 i18n.lang，所以模板里的文字会随切换立即更新；
// 后端产生的文字（检查结果、错误提示）由 app.set_lang 切换，切换后重新拉取即可。

import { execute } from './backend';

export type Lang = 'zh' | 'en';

const KEY = 'yanmo.lang';

function initial(): Lang {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved === 'zh' || saved === 'en') return saved;
  } catch {
    /* 存储不可用时按系统语言 */
  }
  return /^zh/i.test(navigator.language) ? 'zh' : 'en';
}

export const i18n = $state({ lang: initial() });

export function tr(zh: string, en: string): string {
  return i18n.lang === 'zh' ? zh : en;
}

/** 通知后端当前语言；启动时与每次切换时调用。 */
export async function syncBackendLang(): Promise<void> {
  document.documentElement.lang = i18n.lang === 'zh' ? 'zh-CN' : 'en';
  document.title = tr('研墨 YanMo', 'YanMo');
  await execute('app.set_lang', { lang: i18n.lang }).catch(() => {});
}

export async function setLang(lang: Lang): Promise<void> {
  i18n.lang = lang;
  try {
    localStorage.setItem(KEY, lang);
  } catch {
    /* 不记住也能用 */
  }
  await syncBackendLang();
}
