import { describe, expect, test } from "bun:test";
import { buildWidgetSummary } from "./widget-summary";
import type { Project, ProcessRow } from "./types";

describe("buildWidgetSummary", () => {
  const www = { name: "www", path: "/a/app/www", children: [] } as unknown as Project;
  const app = { name: "app", path: "/a/app", children: [www] } as unknown as Project;
  const api = { name: "api", path: "/a/api", children: [] } as unknown as Project;

  test("counts running and crashed projects by folder, not by name", () => {
    const processes = [
      { path: "/a/app/www", status: "Running" },
      { path: "/a/api", status: "Crashed" },
    ] as unknown as ProcessRow[];
    const summary = buildWidgetSummary({
      projects: [app, api],
      processes,
      ports: [{ status: "conflict" }] as never,
      tasks: [
        { status: "todo" },
        { status: "in_progress" },
        { status: "done" },
      ] as never,
      agentRuns: [{ status: "running" }, { status: "needs_review" }] as never,
      now: new Date("2026-11-03T16:00:00Z"),
    });
    expect(summary.version).toBe(1);
    expect(summary.projects).toEqual([
      { name: "app", running: 0, failing: 0, pulse: null },
      { name: "www", running: 1, failing: 0, pulse: null },
      { name: "api", running: 0, failing: 1, pulse: null },
    ]);
    expect(summary.failing_total).toBe(1);
    expect(summary.port_conflicts).toBe(1);
    expect(summary.tasks_todo).toBe(1);
    expect(summary.tasks_in_progress).toBe(1);
    expect(summary.agents_running).toBe(1);
    expect(summary.updated_at).toBe("2026-11-03T16:00:00.000Z");
  });

  test("attaches pulses by project path and leaves others null", () => {
    const summary = buildWidgetSummary({
      projects: [api],
      processes: [],
      ports: [],
      tasks: [],
      agentRuns: [],
      pulses: { "/a/api": { summary: "сборка", eta_seconds: 90 } },
      now: new Date("2026-11-03T16:00:00Z"),
    });
    expect(summary.projects[0].pulse).toEqual({ summary: "сборка", eta_seconds: 90 });
    const empty = buildWidgetSummary({
      projects: [api],
      processes: [],
      ports: [],
      tasks: [],
      agentRuns: [],
      now: new Date("2026-11-03T16:00:00Z"),
    });
    expect(empty.projects[0].pulse).toBeNull();
  });
});
