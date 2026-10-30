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

export interface QuietHoursConfig {
  enabled?: boolean;
  start?: string;
  end?: string;
}

export interface AlertsConfig {
  rules: AlertRule[];
  /** Окно дедупа, секунды (по умолчанию 600). */
  dedupe_window_secs?: number;
  /** Мин. интервал между голосами, секунды (по умолчанию 60). */
  voice_min_interval_secs?: number;
  quiet_hours?: QuietHoursConfig;
  /** IANA TZ, по умолчанию Europe/Berlin. */
  timezone?: string;
}

/** Лимит параллельных агентов и порог памяти (#29). */
export interface AgentsConfig {
  /** Одновременно Running; по умолчанию 1. */
  max_concurrent?: number;
  /** Минимум доступной RAM, МиБ; по умолчанию 2048. */
  min_free_memory_mb?: number;
  /** Запас под Ollama, МиБ, если процесс запущен; по умолчанию 4096. */
  ollama_reserve_mb?: number;
}

export const DEFAULT_AGENT_CONCURRENCY = 1;
export const DEFAULT_MIN_FREE_MEMORY_MB = 2048;
export const DEFAULT_OLLAMA_RESERVE_MB = 4096;

export function agentsConfigOrDefault(
  agents?: AgentsConfig | null,
): Required<AgentsConfig> {
  return {
    max_concurrent: agents?.max_concurrent ?? DEFAULT_AGENT_CONCURRENCY,
    min_free_memory_mb: agents?.min_free_memory_mb ?? DEFAULT_MIN_FREE_MEMORY_MB,
    ollama_reserve_mb: agents?.ollama_reserve_mb ?? DEFAULT_OLLAMA_RESERVE_MB,
  };
}

/** Требуемая свободная RAM с учётом Ollama (зеркало `AgentLimiter::required_free_mb`). */
export function requiredFreeMemoryMb(
  agents: AgentsConfig | undefined,
  ollamaRunning: boolean,
): number {
  const cfg = agentsConfigOrDefault(agents);
  return ollamaRunning
    ? cfg.min_free_memory_mb + cfg.ollama_reserve_mb
    : cfg.min_free_memory_mb;
}

/** Статусы `agent_runs` (#31), зеркало Rust `RunStatus`. */
export type AgentRunStatus =
  | "queued"
  | "running"
  | "needs_review"
  | "failed"
  | "discarded"
  | "accepted";

export const AGENT_RUN_STATUSES: readonly AgentRunStatus[] = [
  "queued",
  "running",
  "needs_review",
  "failed",
  "discarded",
  "accepted",
] as const;

/** Допустимые переходы (зеркало `can_transition`). */
export function canTransitionAgentRun(
  from: AgentRunStatus,
  to: AgentRunStatus,
): boolean {
  const edges: Record<AgentRunStatus, readonly AgentRunStatus[]> = {
    queued: ["running", "discarded"],
    running: ["needs_review", "failed"],
    needs_review: ["accepted", "discarded"],
    failed: ["discarded"],
    discarded: [],
    accepted: [],
  };
  return edges[from].includes(to);
}

/** Running → needs_review / failed по тесту (#31). */
export function statusAfterTests(opts: {
  skipped: boolean;
  exitCode: number | null;
}): AgentRunStatus {
  if (opts.skipped || opts.exitCode === 0) return "needs_review";
  return "failed";
}

export interface AgentTestRunResult {
  command: string;
  exit_code: number | null;
  log: string;
  skipped: boolean;
  duration_ms: number;
}

export interface AgentReviewBundle {
  tests: AgentTestRunResult;
  diff: string;
  base_ref: string;
  status: AgentRunStatus;
}

export interface Settings {
  scan_roots: string[];
  alerts?: AlertsConfig;
  agents?: AgentsConfig;
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
