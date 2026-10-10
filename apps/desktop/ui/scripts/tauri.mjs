// `pnpm tauri <命令>` 的包装：前端目录与 src-tauri 平级，Tauri CLI 默认找不到项目，
// 这里通过 TAURI_APP_PATH 指明位置（跨平台，不依赖 shell 语法）。
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const env = { ...process.env, TAURI_APP_PATH: resolve(here, '../../src-tauri') };
const bin = process.platform === 'win32' ? 'tauri.cmd' : 'tauri';
const r = spawnSync(resolve(here, '../node_modules/.bin', bin), process.argv.slice(2), {
  stdio: 'inherit',
  env,
  shell: process.platform === 'win32',
});
process.exit(r.status ?? 1);
