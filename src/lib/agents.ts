import type { AgentRunRecord, AgentRunStatus } from "./types";
import { canTransitionAgentRun } from "./types";

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
