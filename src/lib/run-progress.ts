import type { AgentRunRecord, AgentRunStatus } from "./types";

/**
 * Прогресс запуска агента по этапам. Агент не отдаёт точный процент, поэтому
 * прогресс — это этап жизненного цикла, а не доля выполненной работы.
 */
export interface RunProgress {
  percent: number;
  label: string;
  tone: "active" | "done" | "failed" | "idle";
  /** Агент работает: полоса пульсирует, потому что конца не видно. */
  indeterminate: boolean;
}

const STAGES: Record<AgentRunStatus, RunProgress> = {
  queued: { percent: 10, label: "В очереди", tone: "idle", indeterminate: false },
  running: { percent: 55, label: "Агент работает", tone: "active", indeterminate: true },
  needs_review: { percent: 100, label: "Готово к проверке", tone: "done", indeterminate: false },
  accepted: { percent: 100, label: "Принято", tone: "done", indeterminate: false },
  failed: { percent: 100, label: "Ошибка", tone: "failed", indeterminate: false },
  discarded: { percent: 100, label: "Отброшено", tone: "idle", indeterminate: false },
};

export function runProgress(run: Pick<AgentRunRecord, "status">): RunProgress {
  return STAGES[run.status] ?? STAGES.queued;
}

/** Самый свежий запуск по задаче. Даты RFC 3339 сравниваются как строки. */
export function latestRunForTask(
  runs: readonly AgentRunRecord[],
  taskId: string,
): AgentRunRecord | undefined {
  let best: AgentRunRecord | undefined;
  for (const run of runs) {
    if (run.task_id !== taskId) continue;
    if (!best || (run.started_at ?? "") > (best.started_at ?? "")) best = run;
  }
  return best;
}
