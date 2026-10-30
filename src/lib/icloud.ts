/** Зеркало путей / санитизации iCloud summary sync (#37). */

export const ICLOUD_DRIVE_CONTAINER = "com~apple~CloudDocs";
export const DEFAULT_ICLOUD_FOLDER = "Demerzel";
export const SUMMARY_FILE = "summary.json";

export interface ICloudSyncConfig {
  enabled?: boolean;
  /** Относительная папка внутри CloudDocs */
  folder?: string;
}

export type SyncOutcome = "skipped_disabled" | "unchanged" | "written";

export interface SyncResult {
  outcome: SyncOutcome;
  path?: string | null;
}

/**
 * Нормализует относительную папку: без `..`, абсолютных и пустых сегментов.
 * Пустая / невалидная → `Demerzel`.
 */
export function sanitizeICloudFolder(folder: string): string {
  const rawInput = folder.trim();
  if (!rawInput) return DEFAULT_ICLOUD_FOLDER;
  // Абсолютный путь (как в Rust: RootDir) → дефолт.
  if (rawInput.startsWith("/") || /^[A-Za-z]:[\\/]/.test(rawInput)) {
    return DEFAULT_ICLOUD_FOLDER;
  }

  const trimmed = rawInput.replace(/^\/+|\/+$/g, "");
  const parts: string[] = [];
  for (const raw of trimmed.split("/")) {
    const seg = raw.trim();
    if (!seg || seg === ".") continue;
    if (seg === ".." || seg.includes("\0")) return DEFAULT_ICLOUD_FOLDER;
    parts.push(seg);
  }
  return parts.length === 0 ? DEFAULT_ICLOUD_FOLDER : parts.join("/");
}

/** Относительный путь `folder/summary.json` после sanitize. */
export function relativeSummaryPath(folder = DEFAULT_ICLOUD_FOLDER): string {
  return `${sanitizeICloudFolder(folder)}/${SUMMARY_FILE}`;
}

export function isICloudSyncEnabled(config: ICloudSyncConfig = {}): boolean {
  return config.enabled === true;
}
