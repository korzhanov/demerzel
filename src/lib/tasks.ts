import type { Task, TaskStatus } from "./types";

/** Колонки доски по wire-статусам модели Task (#16 / #19). */
export const TASK_BOARD_COLUMNS: readonly { status: TaskStatus; label: string }[] = [
  { status: "todo", label: "К выполнению" },
  { status: "in_progress", label: "В работе" },
  { status: "done", label: "Готово" },
  { status: "blocked", label: "Заблокировано" },
] as const;

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

/** Фильтр доски: пустой projectFilter = все проекты (путь проекта). */
export function filterTasksByProject(
  tasks: readonly Task[],
  projectFilter: string,
): Task[] {
  if (!projectFilter) return [...tasks];
  return tasks.filter((task) => task.project === projectFilter);
}

/** Карточки одной колонки доски. */
export function tasksForColumn(
  tasks: readonly Task[],
  status: TaskStatus,
): Task[] {
  return tasks.filter((task) => task.status === status);
}

/**
 * Можно открыть источник: http(s) URL issue или абсолютный путь к файлу
 * (TODO.md / tasks.json).
 */
export function canOpenTask(task: Pick<Task, "url">): boolean {
  const url = task.url?.trim();
  if (!url) return false;
  return /^https?:\/\//.test(url) || url.startsWith("/");
}

/**
 * Кнопка «Исправить» (#33): агент из задачи, кроме уже закрытых (`done`).
 * Закрытие после accept — только вручную.
 */
export function canStartAgentFromTask(task: Pick<Task, "status">): boolean {
  return task.status !== "done";
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

/** Компактная строка сводки доски (мокup: «В работе N · Todo N · Готово N»). */
export function formatTaskBoardSummary(
  counts: ReturnType<typeof countByStatus>,
): string {
  const parts = [
    `В работе ${counts.in_progress}`,
    `Todo ${counts.todo}`,
    `Готово ${counts.done}`,
  ];
  if (counts.blocked > 0) parts.push(`Заблок. ${counts.blocked}`);
  return parts.join(" · ");
}
