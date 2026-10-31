/** Таблица установки CLI-агентов (#140) — зеркало `config/agent-install.json`. */

import type { AgentRunner } from "./types";
import installTable from "../../config/agent-install.json";

export interface AgentInstallRow {
  runner: string;
  binary: string;
  binaries: string[];
  install: string | null;
  install_alt?: string | null;
  update: string | null;
  install_hint: string;
  verified: boolean;
  headless_supported: boolean;
  notes?: string | null;
}

export interface AgentCatalogPref {
  runner: AgentRunner;
  enabled: boolean;
}

export interface AgentInstallTable {
  terminal_default: string;
  agents: AgentInstallRow[];
}

export function loadAgentInstallTable(): AgentInstallTable {
  return installTable as AgentInstallTable;
}

export function installRowFor(runner: AgentRunner): AgentInstallRow | undefined {
  return loadAgentInstallTable().agents.find((a) => a.runner === runner);
}

/** Команда для вставки в Terminal.app (основная, не alt). */
export function buildInstallCommand(runner: AgentRunner): string | null {
  const row = installRowFor(runner);
  if (!row?.install) return null;
  return row.install;
}

/**
 * Порядок «используемых»: prefs если заданы, иначе таблица.
 * Выключенные остаются в списке (чекбокс), но не вызываются.
 */
export function mergeCatalogPrefs(
  prefs: readonly AgentCatalogPref[] | null | undefined,
  runners: readonly AgentRunner[],
): AgentCatalogPref[] {
  const known = new Set(runners);
  const out: AgentCatalogPref[] = [];
  const seen = new Set<string>();

  for (const p of prefs ?? []) {
    if (!known.has(p.runner) || seen.has(p.runner)) continue;
    seen.add(p.runner);
    out.push({ runner: p.runner, enabled: p.enabled });
  }
  for (const r of runners) {
    if (seen.has(r)) continue;
    out.push({ runner: r, enabled: true });
  }
  return out;
}

/** Первый включённый и установленный; иначе null. */
export function pickPreferredRunner(
  prefs: readonly AgentCatalogPref[],
  availableRunners: ReadonlySet<AgentRunner> | readonly AgentRunner[],
): AgentRunner | null {
  const avail =
    availableRunners instanceof Set
      ? availableRunners
      : new Set(availableRunners);
  for (const p of prefs) {
    if (!p.enabled) continue;
    if (avail.has(p.runner)) return p.runner;
  }
  return null;
}

export function reorderCatalog(
  prefs: readonly AgentCatalogPref[],
  fromIndex: number,
  toIndex: number,
): AgentCatalogPref[] {
  if (
    fromIndex < 0 ||
    toIndex < 0 ||
    fromIndex >= prefs.length ||
    toIndex >= prefs.length ||
    fromIndex === toIndex
  ) {
    return [...prefs];
  }
  const next = [...prefs];
  const [item] = next.splice(fromIndex, 1);
  next.splice(toIndex, 0, item);
  return next;
}

export function setCatalogEnabled(
  prefs: readonly AgentCatalogPref[],
  runner: AgentRunner,
  enabled: boolean,
): AgentCatalogPref[] {
  return prefs.map((p) => (p.runner === runner ? { ...p, enabled } : p));
}
