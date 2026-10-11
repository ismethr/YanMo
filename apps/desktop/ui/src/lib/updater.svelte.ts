// 自动更新：启动后检查（每天最多一次），有新版本时在状态栏提示，点击后下载、安装并重启。

import { invoke } from '@tauri-apps/api/core';
import { inTauri } from './backend';
import { tr } from './i18n.svelte';
import { app } from './store.svelte';
import { prefs } from './prefs.svelte';

const LAST_CHECK_KEY = 'yanmo.update.lastCheck';
const DAY = 24 * 60 * 60 * 1000;

interface UpdateInfo {
  version: string;
  current_version: string;
  notes: string | null;
}

class UpdaterState {
  available = $state<UpdateInfo | null>(null);
  /** 下载进度 0–1；null 表示未在下载 */
  progress = $state<number | null>(null);
  checking = $state(false);

  /** 启动后调用：距上次检查不足一天则跳过 */
  async autoCheck() {
    if (!inTauri || !prefs.checkUpdates) return;
    try {
      const last = Number(localStorage.getItem(LAST_CHECK_KEY) ?? 0);
      if (Date.now() - last < DAY) return;
    } catch {
      /* 存储不可用时照常检查 */
    }
    await this.check(true);
  }

  /** quiet 为 true 时，没有更新或检查失败都不提示 */
  async check(quiet = false) {
    if (!inTauri) {
      if (!quiet) app.flash(tr('浏览器预览模式不检查更新', 'Updates are not checked in the browser preview'));
      return;
    }
    if (this.checking) return;
    this.checking = true;
    try {
      this.available = await invoke<UpdateInfo | null>('update_check');
      try {
        localStorage.setItem(LAST_CHECK_KEY, String(Date.now()));
      } catch {
        /* 不记也行 */
      }
      if (this.available) {
        const v = this.available.version;
        app.flash(tr(`研墨 ${v} 可以更新了`, `YanMo ${v} is available`), {
          label: tr('更新并重启', 'Update and Restart'),
          run: () => void this.install(),
        });
      } else if (!quiet) {
        app.flash(tr('已是最新版本', 'YanMo is up to date'));
      }
    } catch (e) {
      if (!quiet) app.flash(tr(`检查更新失败：${e}`, `Could not check for updates: ${e}`));
    } finally {
      this.checking = false;
    }
  }

  async install() {
    if (!this.available || this.progress !== null) return;
    // 先保存，安装完成后应用会重启
    await app.saveAll();
    this.progress = 0;
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ downloaded: number; total: number | null }>('update-progress', (e) => {
      if (e.payload.total) this.progress = e.payload.downloaded / e.payload.total;
    });
    try {
      await invoke('update_install');
    } catch (e) {
      this.progress = null;
      app.flash(tr(`更新失败：${e}`, `Update failed: ${e}`));
    } finally {
      unlisten();
    }
  }
}

export const updater = new UpdaterState();
