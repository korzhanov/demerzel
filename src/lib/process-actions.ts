import type { PortRow, ProcessRow } from "./types";
import { isRunning } from "./types";

/** Какие кнопки доступны в строке процесса. Чистая логика, без Tauri. */
export function processActions(row: Pick<ProcessRow, "status" | "paused" | "port">) {
  const running = isRunning(row.status);
  const paused = running && row.paused === true;
  return {
    start: !running,
    pause: running && !paused,
    resume: paused,
    stop: running,
    restart: true,
    open: running && row.port != null,
  };
}

/** Короткая подпись состояния для бейджа. */
export function processStateLabel(row: Pick<ProcessRow, "status" | "paused">): string {
  if (row.paused && isRunning(row.status)) return "На паузе";
  if (row.status === "Starting") return "Запускается";
  if (isRunning(row.status)) return "Работает";
  if (row.status === "Crashed") return "Упал";
  if (typeof row.status === "object" && row.status !== null && "Exited" in row.status) {
    return `Завершён, код ${row.status.Exited}`;
  }
  return "Остановлен";
}

/** Подпись происхождения: откуда Demerzel знает про процесс. */
export function processOriginLabel(origin: ProcessRow["origin"]): string {
  switch (origin) {
    case "restored":
      return "Восстановлен после перезапуска";
    case "attached":
      return "Присоединён извне";
    default:
      return "Запущен Demerzel";
  }
}

/** Адрес для открытия порта в браузере. */
export function localhostUrl(port: number): string {
  return `http://localhost:${port}`;
}

/** Пояснение, почему логов нет. Присоединённый процесс пишет вывод не в файлы Demerzel. */
export function logsUnavailableHint(origin: ProcessRow["origin"]): string | null {
  if (origin === "attached") {
    return "Вывод присоединённого процесса Demerzel не записывает. Логи есть только у процессов, которые она запустила.";
  }
  return null;
}

/** Порты, которые слушают проекты, но которыми Demerzel не управляет: их можно присоединить. */
export function attachablePorts(ports: readonly PortRow[]): PortRow[] {
  return ports.filter((p) => p.managed_id === null && p.project != null && p.status !== "conflict");
}
