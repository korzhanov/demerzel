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

/** Раннер агента (#28 / #30 / #95). Claude — основной путь. */
export type AgentRunner =
  | "claude"
  | "codex"
  | "gemini"
  | "aider"
  | "opencode"
  | "cursor_agent"
  | "hermes"
  | "grok"
  | "ollama";

export const AGENT_RUNNERS: readonly AgentRunner[] = [
  "claude",
  "codex",
  "gemini",
  "aider",
  "opencode",
  "cursor_agent",
  "hermes",
  "grok",
  "ollama",
] as const;

/** Запись реестра CLI на машине (#95). */
export interface AgentCatalogEntry {
  runner: AgentRunner;
  binary: string;
  available: boolean;
  path?: string | null;
  version?: string | null;
  install_hint: string;
  headless_supported: boolean;
}

/** Режим автофикса по алерту (#96). */
export type AutofixMode = "off" | "suggest" | "auto";

export const AUTOFIX_MODES: readonly AutofixMode[] = [
  "off",
  "suggest",
  "auto",
] as const;

export const DEFAULT_AUTOFIX_MODE: AutofixMode = "off";

/** Лимит параллельных агентов и порог памяти (#29); раннер Ollama (#30); автофикс (#96). */
export interface AgentsConfig {
  /** Одновременно Running; по умолчанию 1. */
  max_concurrent?: number;
  /** Минимум доступной RAM, МиБ; по умолчанию 2048. */
  min_free_memory_mb?: number;
  /** Запас под Ollama, МиБ, если процесс запущен; по умолчанию 4096. */
  ollama_reserve_mb?: number;
  /** Раннер по умолчанию; `claude` если не задан. */
  runner?: AgentRunner;
  /** Модель `ollama run`; по умолчанию qwen2.5-coder:7b. */
  ollama_model?: string;
  /** Автофикс по алерту; по умолчанию `off`. */
  autofix?: AutofixMode;
  /** Порядок и включение агентов (#140). */
  catalog?: { runner: AgentRunner; enabled: boolean }[];
}

export const DEFAULT_AGENT_CONCURRENCY = 1;
export const DEFAULT_MIN_FREE_MEMORY_MB = 2048;
export const DEFAULT_OLLAMA_RESERVE_MB = 4096;
export const DEFAULT_AGENT_RUNNER: AgentRunner = "claude";
export const DEFAULT_OLLAMA_MODEL = "qwen2.5-coder:7b";

export function agentsConfigOrDefault(
  agents?: AgentsConfig | null,
): Required<AgentsConfig> {
  return {
    max_concurrent: agents?.max_concurrent ?? DEFAULT_AGENT_CONCURRENCY,
    min_free_memory_mb: agents?.min_free_memory_mb ?? DEFAULT_MIN_FREE_MEMORY_MB,
    ollama_reserve_mb: agents?.ollama_reserve_mb ?? DEFAULT_OLLAMA_RESERVE_MB,
    runner: agents?.runner ?? DEFAULT_AGENT_RUNNER,
    ollama_model: agents?.ollama_model ?? DEFAULT_OLLAMA_MODEL,
    autofix: agents?.autofix ?? DEFAULT_AUTOFIX_MODE,
    catalog: agents?.catalog ?? [],
  };
}

export function parseAutofixMode(raw: string | null | undefined): AutofixMode {
  const v = (raw ?? "").trim().toLowerCase();
  if (v === "suggest" || v === "offer" || v === "ask") return "suggest";
  if (v === "auto" || v === "automatic") return "auto";
  return "off";
}

export function parseAgentRunner(raw: string | null | undefined): AgentRunner {
  const v = (raw ?? "").trim().toLowerCase();
  if (v === "cursor-agent" || v === "agent") return "cursor_agent";
  if ((AGENT_RUNNERS as readonly string[]).includes(v)) {
    return v as AgentRunner;
  }
  return "claude";
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

/** Запись agent run для UI ревью (#32 / #30). */
export interface AgentRunRecord {
  id: string;
  project: string;
  project_path: string;
  task_id: string | null;
  status: AgentRunStatus;
  worktree: string | null;
  base_ref: string;
  /** Какой CLI; отсутствует → claude. */
  runner?: AgentRunner;
  tests: AgentTestRunResult | null;
  diff: string | null;
  started_at: string | null;
  finished_at: string | null;
}

export interface AcceptAgentRunResult {
  run_id: string;
  status: AgentRunStatus;
  merge_command: string;
}

/** Результат `start_agent_from_task` (#33). */
export interface StartAgentRunResult {
  run: AgentRunRecord;
  task: Task;
  queued: boolean;
  queue_reason: string | null;
}

/** Предложение автофикса из алерта (#96). */
export interface AutofixOffer {
  mode: AutofixMode;
  project: string;
  project_path: string;
  message: string;
  kind: AlertEventKind;
  prompt: string;
  command?: string | null;
}

/** Результат `start_agent_from_alert` (#96). */
export interface StartAgentFromAlertResult {
  run: AgentRunRecord;
  queued: boolean;
  queue_reason: string | null;
  mode: AutofixMode;
}

export type { ScanRootEntry, ScanRootRules } from "$lib/scan-roots";
import type { ScanRootEntry } from "$lib/scan-roots";

export interface Settings {
  /** Включённые корни (зеркало scan_entries). */
  scan_roots: string[];
  /** Дерево выбора + per-folder rules. */
  scan_entries?: ScanRootEntry[];
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

export interface DiscoveredAutomation {
  source: string;
  name: string;
  command: string | null;
  likely_automation: boolean;
}
export interface AutomationSelection {
  source: string;
  name: string;
}

export interface AutomationSchedule extends AutomationSelection {
  interval_minutes: number;
  enabled: boolean;
}
export interface RestartRule {
  enabled: boolean;
  max_retries: number;
}
export interface AutomationsView {
  discovered: DiscoveredAutomation[];
  selected: AutomationSelection[];
  schedules: AutomationSchedule[];
  restart: RestartRule;
}

export interface AutomationStatus {
  source: string;
  name: string;
  command: string | null;
  run_id: number | null;
  status: ProcessStatus | null;
}
