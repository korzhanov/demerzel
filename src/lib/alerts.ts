/** Зеркало severity-хелперов бэкенда (#20). Доставка — #21–#23. */

export type AlertSeverity = "info" | "important" | "critical";

export type AlertEventKind =
  | "process_exited"
  | "port_conflict"
  | "agent_failed"
  | "tests_failed";

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
