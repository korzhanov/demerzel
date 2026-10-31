import type { AutomationSelection, AutomationsView, DiscoveredAutomation } from "./types";

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
