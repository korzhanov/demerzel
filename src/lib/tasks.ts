import type { Task, TaskStatus } from "./types";

/** Подписи статусов нормализованной модели задачи (#16). */
export function taskStatusLabel(status: TaskStatus): string {
  switch (status) {
    case "todo":
      return "todo";
    case "in_progress":
      return "in_progress";
    case "done":
      return "done";
    case "blocked":
      return "blocked";
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

/** Сводка по статусам — зеркало `count_by_status` на бэкенде. */
export function countByStatus(tasks: readonly Pick<Task, "status">[]): {
  todo: number;
  in_progress: number;
  done: number;
  blocked: number;
} {
  const out = { todo: 0, in_progress: 0, done: 0, blocked: 0 };
  for (const task of tasks) {
    out[task.status] += 1;
  }
  return out;
}
