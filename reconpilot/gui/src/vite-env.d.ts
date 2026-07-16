/// <reference types="vite/client" />

declare global {
  interface Window {
    __TAURI__?: {
      core?: {
        invoke<T>(command: string, payload?: Record<string, unknown>): Promise<T>;
      };
    };
  }
}

export {};
