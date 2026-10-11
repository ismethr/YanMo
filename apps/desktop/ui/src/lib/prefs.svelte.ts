// 应用偏好（与项目无关，按用户记在本机）：编辑器、编译行为、更新检查。
// 项目相关的设置（编译引擎、shell-escape、主文件）存在项目的 .yanmo/project.toml，见 project.settings 命令。

const KEY = 'yanmo.prefs';

export interface Prefs {
  /** 编辑器字号（px） */
  fontSize: number;
  /** 长行自动换行 */
  wrap: boolean;
  /** 停止输入后自动保存 */
  autosave: boolean;
  /** 保存后自动编译 */
  autoCompile: boolean;
  /** 编译超时（秒），0 表示不限 */
  compileTimeout: number;
  /** 启动时检查更新 */
  checkUpdates: boolean;
}

export const DEFAULT_PREFS: Prefs = {
  fontSize: 13.5,
  wrap: true,
  autosave: true,
  autoCompile: true,
  compileTimeout: 300,
  checkUpdates: true,
};

function load(): Prefs {
  try {
    return { ...DEFAULT_PREFS, ...JSON.parse(localStorage.getItem(KEY) ?? '{}') };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

export const prefs = $state<Prefs>(load());

/** 修改后调用以保存 */
export function savePrefs() {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    /* 存储不可用时只在本次运行中生效 */
  }
}
