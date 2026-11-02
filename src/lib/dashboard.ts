import type { PortRow, ProcessRow, ProcessStatus, Project } from "./types";
import { isRunning, statusLabel } from "./types";

/** Цвет точки статуса проекта в сайдбаре (мокup columns). */
export type DotTone = "ok" | "bad" | "idle" | "warn";

export interface ProjectRowStatus {
  tone: DotTone;
  label: string;
}

export interface DashboardSummary {
  projectCount: number;
  crashedCount: number;
  portConflictCount: number;
  taskCount: number;
}

/** Плоский список проектов из дерева скана. */
export function flattenProjects(list: readonly Project[]): Project[] {
  return [
    ...new Map(
      list.flatMap((p) => [p, ...flattenProjects(p.children)]).map((p) => [p.path, p]),
    ).values(),
  ];
}

/**
 * Процесс проекта. Сопоставление по папке: имена подпроектов (www, frontend) повторяются,
 * и по имени чужой процесс подсветился бы как запущенный.
 */
export function processForProject(
  processes: readonly ProcessRow[],
  project: Pick<Project, "path">,
): ProcessRow | undefined {
  return processes.find((row) => row.path === project.path);
}

function isCrashedStatus(status: ProcessStatus): boolean {
  if (status === "Crashed") return true;
  if (typeof status === "object" && "Exited" in status) return status.Exited !== 0;
  return false;
}

/** Точка + подпись справа в строке проекта (мокup: `:5174` / `Exited 1` / `нет конфига`). */
export function projectRowStatus(
  project: Pick<Project, "path">,
  processes: readonly ProcessRow[],
): ProjectRowStatus {
  const row = processForProject(processes, project);
  if (!row) return { tone: "idle", label: "нет процесса" };
  if (isRunning(row.status)) {
    return {
      tone: "ok",
      label: row.port != null ? `:${row.port}` : statusLabel(row.status),
    };
  }
  if (typeof row.status === "object" && "Exited" in row.status) {
    return {
      tone: row.status.Exited === 0 ? "idle" : "bad",
      label: `Exited ${row.status.Exited}`,
    };
  }
  if (row.status === "Crashed") return { tone: "bad", label: "Crashed" };
  if (row.status === "Stopped") return { tone: "idle", label: "Stopped" };
  return { tone: "idle", label: statusLabel(row.status) };
}

/** Сводка шапки: проектов / упало / конфликт портов / задач. */
export function buildDashboardSummary(input: {
  projects: readonly Project[];
  processes: readonly ProcessRow[];
  ports: readonly PortRow[];
  tasks: readonly unknown[];
}): DashboardSummary {
  const flat = flattenProjects(input.projects);
  return {
    projectCount: flat.length,
    crashedCount: input.processes.filter((row) => isCrashedStatus(row.status)).length,
    portConflictCount: input.ports.filter((row) => row.status === "conflict").length,
    taskCount: input.tasks.length,
  };
}

/** Время «обновлено HH:MM» как на мокапе. */
export function formatUpdatedAt(date: Date): string {
  return date.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
}

/** Tailwind-класс точки статуса. */
export function dotToneClass(tone: DotTone): string {
  switch (tone) {
    case "ok":
      return "bg-[var(--status-ok)]";
    case "bad":
      return "bg-destructive";
    case "warn":
      return "bg-primary";
    case "idle":
    default:
      return "bg-muted-foreground";
  }
}

/** Цвет текста статуса справа. */
export function statusToneClass(tone: DotTone): string {
  switch (tone) {
    case "ok":
      return "text-[var(--status-ok)]";
    case "bad":
      return "text-destructive";
    case "warn":
      return "text-primary";
    case "idle":
    default:
      return "text-muted-foreground";
  }
}
