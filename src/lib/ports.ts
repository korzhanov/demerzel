import type { PortRow } from "./types";

export type PortStatus = PortRow["status"];

/** Подписи статусов инвентаря портов для таблицы UI. */
export function portStatusLabel(status: PortStatus): string {
  switch (status) {
    case "ok":
      return "ok";
    case "conflict":
      return "конфликт";
    case "foreign":
      return "чужой";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

/** Остановить можно только процесс, который запустил оркестратор. */
export function canStopManagedPort(row: Pick<PortRow, "managed_id">): boolean {
  return row.managed_id !== null;
}

/** Интервал опроса портов, секунды (1–60). */
export function clampPortPollInterval(raw: number): number {
  if (!Number.isFinite(raw)) return 5;
  return Math.max(1, Math.min(60, Math.trunc(raw)));
}
