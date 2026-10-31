import type {
  AutomationSelection,
  AutomationStatus,
  AutomationsView,
  DiscoveredAutomation,
} from "./types";
import { isRunning, statusLabel } from "./types";
import type { ProcessStatus, RestartRule } from "./types";

const key = (s: { source: string; name: string }) => `${s.source}\u0000${s.name}`;

/** Начальный выбор: сохранённый; если ничего не сохранено — эвристика likely_automation (#153). */
export function initialSelection(view: AutomationsView): AutomationSelection[] {
  const known = new Set(view.discovered.map(key));
  const saved = view.selected.filter((s) => known.has(key(s)));
  if (view.selected.length > 0) return saved;
  return view.discovered
    .filter((d) => d.likely_automation)
    .map((d) => ({ source: d.source, name: d.name }));
}

export function isChecked(selected: AutomationSelection[], item: DiscoveredAutomation): boolean {
  const k = key(item);
  return selected.some((s) => key(s) === k);
}

export function toggle(selected: AutomationSelection[], item: DiscoveredAutomation): AutomationSelection[] {
  return isChecked(selected, item)
    ? selected.filter((s) => key(s) !== key(item))
    : [...selected, { source: item.source, name: item.name }];
}

/** Текст статуса автозадачи для UI (#153, этап B). */
export function automationStatusText(s: AutomationStatus): string {
  if (!s.command) return "не найдена в проекте";
  if (!s.status) return "не запускалась";
  return statusLabel(s.status);
}

export function automationIsRunning(s: AutomationStatus): boolean {
  return s.status !== null && isRunning(s.status);
}

/** Зеркало backend `should_restart` (#153, этап D). */
export function shouldRestart(status: ProcessStatus | null, attempts: number, rule: RestartRule): boolean {
  if (!rule.enabled || attempts >= rule.max_retries || status === null) return false;
  if (status === "Crashed") return true;
  if (typeof status === "object") return status.Exited !== 0;
  return false;
}
