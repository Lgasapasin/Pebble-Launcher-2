// Type-only fallback for checking UI with a globally installed TypeScript compiler.
// The actual package types are used by npm run check after npm install.
declare module '@tauri-apps/api/core' {
  export function invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T>;
}
declare module '@tauri-apps/api/event' {
  export function listen<T>(event: string, handler: (event: {payload:T}) => void): Promise<() => void>;
}
declare module '*.css';
