/** Зеркало severity + delivery/notify/speak-хелперов бэкенда (#20–#23). */

/** Имя события Tauri после клика по системному уведомлению. */
export const OPEN_PROJECT_EVENT = "alerts:open-project";

export type AlertSeverity = "info" | "important" | "critical";

export type AlertEventKind =
  | "process_exited"
  | "port_conflict"
  | "agent_failed"
  | "tests_failed";

export interface QuietHoursConfig {
  enabled?: boolean;
  /** HH:MM локально в timezone */
  start?: string;
  end?: string;
}

export interface DeliveryDecision {
  emit: boolean;
  speak: boolean;
  notify: boolean;
}

export const DELIVERY_SUPPRESSED: DeliveryDecision = {
  emit: false,
  speak: false,
  notify: false,
};

export function shouldSpeak(severity: AlertSeverity): boolean {
  return severity === "critical";
}

export function shouldNotify(severity: AlertSeverity): boolean {
  return severity === "important" || severity === "critical";
}

export function severityLabel(severity: AlertSeverity): string {
  switch (severity) {
    case "critical":
      return "Критично";
    case "important":
      return "Важно";
    case "info":
      return "Инфо";
  }
}

/** Минуты от полуночи для `HH:MM`; `null` при невалидной строке. */
export function parseHhMmToMinutes(hhmm: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(hhmm.trim());
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  if (h > 23 || min > 59) return null;
  return h * 60 + min;
}

/**
 * Тихие часы в локальных минутах. Окно через полночь (23:00–08:00): start > end.
 */
export function isQuietHoursLocal(
  nowMinutes: number,
  startMinutes: number,
  endMinutes: number,
): boolean {
  if (startMinutes === endMinutes) return false;
  if (startMinutes < endMinutes) {
    return nowMinutes >= startMinutes && nowMinutes < endMinutes;
  }
  return nowMinutes >= startMinutes || nowMinutes < endMinutes;
}

export function isQuietHours(
  nowMinutes: number,
  quiet: QuietHoursConfig = {},
): boolean {
  if (quiet.enabled === false) return false;
  const start = parseHhMmToMinutes(quiet.start ?? "23:00");
  const end = parseHhMmToMinutes(quiet.end ?? "08:00");
  if (start === null || end === null) return false;
  return isQuietHoursLocal(nowMinutes, start, end);
}

/** Флаги speak/notify после severity + тихих часов + троттлинга голоса. */
export function deliveryFlags(
  severity: AlertSeverity,
  opts: { quiet: boolean; voiceThrottled?: boolean },
): Pick<DeliveryDecision, "speak" | "notify"> {
  let speak = shouldSpeak(severity);
  let notify = shouldNotify(severity);
  if (opts.quiet) {
    speak = false;
    if (severity !== "critical") notify = false;
  }
  if (opts.voiceThrottled) speak = false;
  return { speak, notify };
}

/** Короткий заголовок OS-уведомления (зеркало `notify::notify_title`). */
export function notifyTitle(severity: AlertSeverity): string {
  switch (severity) {
    case "critical":
      return "Demerzel · Критично";
    case "important":
      return "Demerzel · Важно";
    case "info":
      return "Demerzel";
  }
}

/**
 * Нужно ли слать OS-notify: только если gate сказал `notify`
 * (severity + тихие часы уже учтены в `deliveryFlags` / DeliveryGate).
 */
export function shouldDeliverNotify(decision: Pick<DeliveryDecision, "notify">): boolean {
  return decision.notify;
}

/**
 * Нужно ли вызывать `say`: только если gate сказал `speak`
 * (critical + тихие часы + троттлинг ≤1/мин).
 */
export function shouldDeliverSpeak(decision: Pick<DeliveryDecision, "speak">): boolean {
  return decision.speak;
}

export type VoicePhraseKind =
  | "process_exited"
  | "port_conflict"
  | "agent_failed"
  | "tests_failed";

export const DEFAULT_VOICE_NAME = "Milena";

export const DEFAULT_VOICE_PHRASES: Record<VoicePhraseKind, string> = {
  process_exited: "Олег, внимание. {project} упал, код {code}.",
  port_conflict: "Олег, внимание. Порт {port} занят.",
  agent_failed: "Олег, внимание. Агент {run_id} не справился.",
  tests_failed: "Олег, внимание. Тесты {run_id} упали.",
};

export type SpeakEventVars = {
  project?: string;
  code?: number | string;
  port?: number | string;
  run_id?: string;
};

/** Подстановка плейсхолдеров в шаблон фразы (зеркало `speak::render_phrase`). */
export function renderVoicePhrase(template: string, vars: SpeakEventVars): string {
  return template
    .replaceAll("{project}", vars.project ?? "")
    .replaceAll("{code}", vars.code != null ? String(vars.code) : "")
    .replaceAll("{port}", vars.port != null ? String(vars.port) : "")
    .replaceAll("{run_id}", vars.run_id ?? "");
}

/** Выбор процесса по имени проекта из уведомления. */
export function processIdForProject(
  processes: ReadonlyArray<{ id: number; project: string }>,
  project: string,
): number | null {
  const hit = processes.find((p) => p.project === project);
  return hit ? hit.id : null;
}
