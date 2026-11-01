// Номер сборки, подставляется Vite (vite.config.js → define). Версия из VERSION, коммит — короткий хеш (#build-label).
declare const __APP_VERSION__: string;
declare const __APP_COMMIT__: string;

export const APP_VERSION: string = __APP_VERSION__;
export const APP_COMMIT: string = __APP_COMMIT__;

/** Строка для интерфейса: `v0.55.25` или `v0.55.25 · a1b2c3d`. */
export function buildLabel(): string {
  return APP_COMMIT ? `v${APP_VERSION} · ${APP_COMMIT}` : `v${APP_VERSION}`;
}
