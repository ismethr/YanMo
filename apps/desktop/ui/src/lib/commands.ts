// 前端唯一的后端调用入口，与 Rust 侧 ym-commands 注册表一一对应。
import { invoke } from "@tauri-apps/api/core";

export function execute<T = unknown>(id: string, params: Record<string, unknown> = {}): Promise<T> {
  return invoke<T>("execute", { id, params });
}
