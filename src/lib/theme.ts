/** Светлая / тёмная тема оформления (класс на <html>). */

export type ThemeMode = "dark" | "light";

const STORAGE_KEY = "demerzel-theme";

export function parseThemeValue(raw: string | null | undefined): ThemeMode {
  return raw === "light" ? "light" : "dark";
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
  const root = document.documentElement;
  root.classList.remove("dark", "light");
  root.classList.add(mode);
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    /* private mode */
  }
}

export function toggleTheme(current: ThemeMode): ThemeMode {
  const next: ThemeMode = current === "dark" ? "light" : "dark";
  applyTheme(next);
  return next;
}
