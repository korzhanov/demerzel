/** Зеркало путей / флагов native WidgetKit fallback (#38). */

export const APP_GROUP_ID = "group.app.demerzel.orchestrator";
export const SUMMARY_FILE = "summary.json";
export const WIDGET_KIND = "DemerzelNativeWidget";

export interface WidgetKitFallbackConfig {
  enabled?: boolean;
}

export type SyncOutcome = "skipped_disabled" | "unchanged" | "written";

export interface SyncResult {
  outcome: SyncOutcome;
  path?: string | null;
}

export function isWidgetKitFallbackEnabled(
  config: WidgetKitFallbackConfig = {},
): boolean {
  return config.enabled === true;
}

/** Относительный путь внутри Group Containers. */
export function relativeAppGroupSummaryPath(
  appGroup = APP_GROUP_ID,
): string {
  return `${appGroup}/${SUMMARY_FILE}`;
}

/**
 * Абсолютный путь как на macOS (без проверки существования контейнера).
 * `home` — `$HOME`, без завершающего `/`.
 */
export function absoluteAppGroupSummaryPath(
  home: string,
  appGroup = APP_GROUP_ID,
): string {
  const base = home.replace(/\/+$/, "");
  return `${base}/Library/Group Containers/${appGroup}/${SUMMARY_FILE}`;
}
