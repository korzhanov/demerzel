import type {
  AgentCatalogEntry,
  AgentRunner,
  AgentRunRecord,
  AgentRunStatus,
  AutofixMode,
} from "./types";
import { canTransitionAgentRun } from "./types";

/** Подпись режима автофикса (#96). */
export function autofixModeLabel(mode: AutofixMode | undefined | null): string {
  switch (mode ?? "off") {
    case "off":
      return "Автофикс выкл";
    case "suggest":
      return "Предлагать фикс";
    case "auto":
      return "Автофикс";
    default: {
      const _exhaustive: never = mode as never;
      return _exhaustive;
    }
  }
}

/** Подпись раннера для UI (#30 / #95). */
export function agentRunnerLabel(runner: AgentRunner | undefined | null): string {
  switch (runner ?? "claude") {
    case "ollama":
      return "Ollama (локально)";
    case "acp":
      return "ACP-агент (agents.acp_command)";
    case "claude":
      return "Claude CLI";
    case "codex":
      return "Codex CLI";
    case "gemini":
      return "Gemini CLI";
    case "aider":
      return "Aider";
    case "opencode":
      return "OpenCode";
    case "cursor_agent":
      return "Cursor Agent";
    case "hermes":
      return "Hermes";
    case "grok":
      return "Grok CLI";
    default: {
      const _exhaustive: never = runner as never;
      return _exhaustive;
    }
  }
}

/** Подпись опции селекта с учётом реестра (#95). */
export function agentRunnerOptionLabel(
  runner: AgentRunner,
  catalog: AgentCatalogEntry[] | null | undefined,
): string {
  const base = agentRunnerLabel(runner);
  const entry = catalog?.find((e) => e.runner === runner);
  if (!entry) return base;
  if (!entry.available) return `${base} (не установлен)`;
  if (!entry.headless_supported) return `${base} (только детект)`;
  if (entry.version) return `${base} · ${entry.version}`;
  return base;
}

/** Подсказка установки для выбранного раннера (#95). */
export function agentInstallHint(
  runner: AgentRunner,
  catalog: AgentCatalogEntry[] | null | undefined,
): string | null {
  const entry = catalog?.find((e) => e.runner === runner);
  if (!entry) return null;
  if (!entry.available) return entry.install_hint;
  if (!entry.headless_supported) {
    return `${entry.install_hint} Headless-прогон пока у Claude/Ollama.`;
  }
  return null;
}

/** Подписи статусов agent run для UI (#32). */
export function agentRunStatusLabel(status: AgentRunStatus): string {
  switch (status) {
    case "queued":
      return "в очереди";
    case "running":
      return "работает";
    case "needs_review":
      return "на ревью";
    case "failed":
      return "ошибка";
    case "discarded":
      return "отклонён";
    case "accepted":
      return "принят";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

export function canAcceptAgentRun(status: AgentRunStatus): boolean {
  return canTransitionAgentRun(status, "accepted");
}

export function canDiscardAgentRun(status: AgentRunStatus): boolean {
  return canTransitionAgentRun(status, "discarded");
}

/** Экранирование пути для single-quoted shell (зеркало Rust). */
export function shellSingleQuote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

/**
 * Команда ручного merge ветки `agent/<run-id>`.
 * UI только показывает её — сам merge не выполняется.
 */
export function manualMergeCommand(repoPath: string, runId: string): string {
  const id = runId.trim();
  if (!id) throw new Error("run_id must not be empty");
  if (!repoPath.trim()) throw new Error("repo path must not be empty");
  return `git -C ${shellSingleQuote(repoPath)} merge --no-ff agent/${id}`;
}

/** Есть ли у записи diff или лог тестов для просмотра. */
export function hasAgentRunReviewPayload(
  run: Pick<AgentRunRecord, "diff" | "tests">,
): boolean {
  const hasDiff = Boolean(run.diff && run.diff.length > 0);
  const hasLog = Boolean(run.tests?.log && run.tests.log.length > 0);
  return hasDiff || hasLog;
}
