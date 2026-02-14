declare module "@tauri-apps/api/core" {
  export function invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T>;
}

declare module "@tauri-apps/api/core.js" {
  export function invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T>;
}
