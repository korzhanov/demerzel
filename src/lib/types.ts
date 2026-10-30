// Типы, которые отдают команды Tauri (src-tauri/src/lib.rs).

export type ProcessStatus =
  | "Stopped"
  | "Starting"
  | "Running"
  | "Crashed"
  | { Exited: number };

export interface Project {
  name: string;
  path: string;
  children: Project[];
}

export interface ProcessRow {
  id: number;
  project: string;
  command: string;
  status: ProcessStatus;
  pid?: number | null;
  port?: number | null;
}

export interface LogLine {
  stream: "stdout" | "stderr";
  text: string;
}

export function statusLabel(s: ProcessStatus): string {
  if (typeof s === "string") return s;
  return `Exited(${s.Exited})`;
}

export function isRunning(s: ProcessStatus): boolean {
  return s === "Running" || s === "Starting";
}

export type AlertSeverity = "info" | "important" | "critical";
export type AlertEventKind =
  | "process_exited"
  | "port_conflict"
  | "agent_failed"
  | "tests_failed";

export interface AlertRuleWhen {
  op: "always" | "exit_code_non_zero";
}

export interface AlertRule {
  id: string;
  kind: AlertEventKind;
  when?: AlertRuleWhen;
  severity: AlertSeverity;
  enabled?: boolean;
}

export interface AlertsConfig {
  rules: AlertRule[];
}

export interface Settings {
  scan_roots: string[];
  alerts?: AlertsConfig;
}

export interface PortRow {
  port: number;
  project: string | null;
  command: string;
  pid: number;
  status: "ok" | "conflict" | "foreign";
  managed_id: number | null;
}

export type TaskStatus = "todo" | "in_progress" | "done" | "blocked";
export interface Task {
  id: string;
  project: string;
  title: string;
  status: TaskStatus;
  source: string;
  url: string | null;
}
