/** Тема оформления: тёмная / светлая / как в системе (класс на <html>). */

export type ThemeMode = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

const STORAGE_KEY = "demerzel-theme";
const CYCLE: readonly ThemeMode[] = ["dark", "light", "system"];

export function parseThemeValue(raw: string | null | undefined): ThemeMode {
  if (raw === "light" || raw === "system" || raw === "dark") return raw;
  return "dark";
}

export function systemPrefersDark(): boolean {
  if (typeof matchMedia === "undefined") return true;
  return matchMedia("(prefers-color-scheme: dark)").matches;
}

export function resolveTheme(
  mode: ThemeMode,
  prefersDark: boolean = systemPrefersDark(),
): ResolvedTheme {
  if (mode === "system") return prefersDark ? "dark" : "light";
  return mode;
}

export function themeLabel(mode: ThemeMode): string {
  switch (mode) {
    case "dark":
      return "Тёмная тема";
    case "light":
      return "Светлая тема";
    case "system":
      return "Системная тема";
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

/** Следующий режим в цикле dark → light → system. */
export function nextTheme(current: ThemeMode): ThemeMode {
  const i = CYCLE.indexOf(current);
  return CYCLE[(i < 0 ? 0 : i + 1) % CYCLE.length]!;
}

export function readStoredTheme(): ThemeMode {
  if (typeof localStorage === "undefined") return "dark";
  try {
    return parseThemeValue(localStorage.getItem(STORAGE_KEY));
  } catch {
    return "dark";
  }
}

export function applyTheme(mode: ThemeMode): void {
  if (typeof document === "undefined") return;
  const resolved = resolveTheme(mode);
  const root = document.documentElement;
  root.classList.remove("dark", "light");
  root.classList.add(resolved);
  root.dataset.theme = mode;
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    /* private mode */
  }
}

export function toggleTheme(current: ThemeMode): ThemeMode {
  const next = nextTheme(current);
  applyTheme(next);
  return next;
}
