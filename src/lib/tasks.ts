import type { Project, Task, TaskStatus } from "./types";

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

/** Путь с завершающим разделителем: `/a/web` → `/a/web/`, чтобы `/a/web2` не попадал в `/a/web`. */
function withSeparator(path: string): string {
  return path.endsWith("/") ? path : `${path}/`;
}

/**
 * Фильтр доски: пустой projectFilter = все проекты. Фильтр — проект вместе с подпроектами,
 * потому что у подпроекта может не быть своей доски.
 */
export function filterTasksByProject(
  tasks: readonly Task[],
  projectFilter: string,
): Task[] {
  if (!projectFilter) return [...tasks];
  const prefix = withSeparator(projectFilter);
  return tasks.filter(
    (task) => task.project === projectFilter || task.project.startsWith(prefix),
  );
}

function containsPath(project: Project, path: string): boolean {
  return project.path === path || project.children.some((c) => containsPath(c, path));
}

/**
 * Путь корневого проекта (верхнего уровня), в дереве которого лежит `path`.
 * Если проект не найден, возвращает сам `path`.
 */
export function boardRootPath(projects: readonly Project[], path: string): string {
  if (!path) return "";
  return projects.find((root) => containsPath(root, path))?.path ?? path;
}

/** Карточки одной колонки доски. */
export function tasksForColumn(
  tasks: readonly Task[],
  status: TaskStatus,
): Task[] {
  return tasks.filter((task) => task.status === status);
}

/** Сколько карточек показывать до спойлера в колонке. */
export const TASK_COLUMN_PREVIEW = 3;

/** Первые `limit` задач видимы, остальные — под спойлером. */
export function splitColumnPreview<T>(
  items: readonly T[],
  limit: number = TASK_COLUMN_PREVIEW,
): { preview: T[]; rest: T[] } {
  const n = Math.max(0, Math.floor(limit));
  if (items.length <= n) return { preview: [...items], rest: [] };
  return { preview: items.slice(0, n), rest: items.slice(n) };
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

/**
 * Убирает Markdown-ссылки из заголовка задачи (#118).
 * `[#35](https://github.com/x/y/issues/35) Квоты` → `#35 Квоты`.
 */
export function cleanTaskTitle(title: string): string {
  return title
    .replace(/\[([^\]]+)\]\((?:https?:\/\/|\/)[^)\s]*\)/g, "$1")
    .replace(/https?:\/\/\S+/g, "")
    .replace(/\s+/g, " ")
    .trim();
}
