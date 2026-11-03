import { flattenProjects, isCrashedStatus, processForProject } from "./dashboard";
import type { AgentRunRecord, PortRow, ProcessRow, Project, Task } from "./types";
import { isRunning } from "./types";

/** Статус проекта в формате виджета (`summary::ProjectStatus` в Rust). */
export interface WidgetProjectStatus {
  name: string;
  running: number;
  failing: number;
  pulse: null;
}

/** Сводка для виджета (`summary::Summary` в Rust, `SUMMARY_VERSION` = 1). */
export interface WidgetSummary {
  version: 1;
  projects: WidgetProjectStatus[];
  failing_total: number;
  port_conflicts: number;
  tasks_todo: number;
  tasks_in_progress: number;
  agents_running: number;
  updated_at: string;
}

/** Собирает сводку, которую `refresh_widget_from_summary` кладёт в App Group для виджета. */
export function buildWidgetSummary(input: {
  projects: readonly Project[];
  processes: readonly ProcessRow[];
  ports: readonly PortRow[];
  tasks: readonly Task[];
  agentRuns: readonly AgentRunRecord[];
  now: Date;
}): WidgetSummary {
  const projects = flattenProjects(input.projects).map((project) => {
    const row = processForProject(input.processes, project);
    return {
      name: project.name,
      running: row && isRunning(row.status) ? 1 : 0,
      failing: row && isCrashedStatus(row.status) ? 1 : 0,
      pulse: null,
    };
  });
  return {
    version: 1,
    projects,
    failing_total: projects.reduce((sum, p) => sum + p.failing, 0),
    port_conflicts: input.ports.filter((row) => row.status === "conflict").length,
    tasks_todo: input.tasks.filter((t) => t.status === "todo").length,
    tasks_in_progress: input.tasks.filter((t) => t.status === "in_progress").length,
    agents_running: input.agentRuns.filter((r) => r.status === "running").length,
    updated_at: input.now.toISOString(),
  };
}
