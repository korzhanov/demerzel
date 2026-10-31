/** Модель корней сканирования: дерево, чекбоксы, приоритет правил вложенных папок. */

export interface ScanRootRules {
  /** Переопределение глобального MAX_DEPTH (fs walk). */
  max_depth?: number | null;
  /** Переопределение MAX_NESTING (подпроекты). */
  max_nesting?: number | null;
  /** Доп. имена каталогов для игнора (поверх встроенных). */
  ignore?: string[];
}

export interface ScanRootEntry {
  path: string;
  enabled: boolean;
  /** Если задано — перекрывает правила предков для путей под этим корнем. */
  rules?: ScanRootRules | null;
}

export type OrchestrationStatus = "configured" | "missing" | "unknown";

export interface DirChild {
  name: string;
  path: string;
  is_dir: boolean;
  orchestration: OrchestrationStatus;
}

export function normalizePath(path: string): string {
  return path.trim().replace(/\/+$/, "") || "/";
}

export function pathsEqual(a: string, b: string): boolean {
  return normalizePath(a) === normalizePath(b);
}

/** `child` лежит строго внутри `parent` или совпадает с ним. */
export function isPathUnder(child: string, parent: string): boolean {
  const c = normalizePath(child);
  const p = normalizePath(parent);
  if (c === p) return true;
  return c.startsWith(`${p}/`);
}

/**
 * Самый специфичный (длинный префикс) entry, покрывающий `path`.
 * У него приоритет правил над предками.
 */
export function resolveEntryForPath(
  path: string,
  entries: readonly ScanRootEntry[],
): ScanRootEntry | null {
  let best: ScanRootEntry | null = null;
  let bestLen = -1;
  for (const e of entries) {
    if (!isPathUnder(path, e.path)) continue;
    const len = normalizePath(e.path).length;
    if (len > bestLen) {
      best = e;
      bestLen = len;
    }
  }
  return best;
}

/** Эффективные правила: от самого специфичного entry с `rules`. */
export function resolveRulesForPath(
  path: string,
  entries: readonly ScanRootEntry[],
): ScanRootRules | null {
  const covering = entries
    .filter((e) => isPathUnder(path, e.path) && e.rules)
    .sort((a, b) => normalizePath(b.path).length - normalizePath(a.path).length);
  return covering[0]?.rules ?? null;
}

export function entriesFromLegacyRoots(roots: readonly string[]): ScanRootEntry[] {
  const out: ScanRootEntry[] = [];
  for (const r of roots) {
    const path = normalizePath(r);
    if (!path || out.some((e) => pathsEqual(e.path, path))) continue;
    out.push({ path, enabled: true, rules: null });
  }
  return out;
}

export function enabledRootPaths(entries: readonly ScanRootEntry[]): string[] {
  return entries.filter((e) => e.enabled).map((e) => normalizePath(e.path));
}

export function allEnabled(entries: readonly ScanRootEntry[]): boolean {
  return entries.length > 0 && entries.every((e) => e.enabled);
}

export function noneEnabled(entries: readonly ScanRootEntry[]): boolean {
  return entries.length === 0 || entries.every((e) => !e.enabled);
}

export function setAllEnabled(
  entries: readonly ScanRootEntry[],
  enabled: boolean,
): ScanRootEntry[] {
  return entries.map((e) => ({ ...e, enabled }));
}

export function upsertEntry(
  entries: readonly ScanRootEntry[],
  path: string,
  patch: Partial<ScanRootEntry> = {},
): ScanRootEntry[] {
  const norm = normalizePath(path);
  const idx = entries.findIndex((e) => pathsEqual(e.path, norm));
  if (idx < 0) {
    return [...entries, { enabled: true, rules: null, ...patch, path: norm }];
  }
  const next = [...entries];
  next[idx] = { ...next[idx], ...patch, path: norm };
  return next;
}

export function removeEntry(
  entries: readonly ScanRootEntry[],
  path: string,
): ScanRootEntry[] {
  return entries.filter((e) => !pathsEqual(e.path, path));
}

export function hasCustomRules(entry: ScanRootEntry | null | undefined): boolean {
  if (!entry?.rules) return false;
  const r = entry.rules;
  return (
    r.max_depth != null ||
    r.max_nesting != null ||
    (r.ignore != null && r.ignore.length > 0)
  );
}
