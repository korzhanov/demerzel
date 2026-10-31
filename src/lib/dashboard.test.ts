import { describe, expect, test } from "bun:test";
import {
  buildDashboardSummary,
  flattenProjects,
  formatUpdatedAt,
  processForProject,
  projectRowStatus,
} from "./dashboard";
import type { PortRow, ProcessRow, Project } from "./types";

function project(name: string, path: string, children: Project[] = []): Project {
  return { name, path, children };
}

function proc(partial: Partial<ProcessRow> & Pick<ProcessRow, "id" | "project" | "status">): ProcessRow {
  return {
    command: "bun run dev",
    ...partial,
  };
}

function port(partial: Partial<PortRow> & Pick<PortRow, "port" | "pid" | "status">): PortRow {
  return {
    command: "vite",
    project: null,
    managed_id: null,
    ...partial,
  };
}

describe("flattenProjects", () => {
  test("дедупит по path и разворачивает дерево", () => {
    const tree = [
      project("root", "/a", [project("www", "/a/www"), project("parser", "/a/parser")]),
      project("demerzel", "/b"),
    ];
    expect(flattenProjects(tree).map((p) => p.path)).toEqual([
      "/a",
      "/a/www",
      "/a/parser",
      "/b",
    ]);
  });
});

describe("projectRowStatus", () => {
  test("idle без процесса", () => {
    expect(projectRowStatus({ name: "www" }, [])).toEqual({
      tone: "idle",
      label: "нет процесса",
    });
  });

  test("ok с портом", () => {
    const processes = [proc({ id: 1, project: "www", status: "Running", port: 5174 })];
    expect(projectRowStatus({ name: "www" }, processes)).toEqual({
      tone: "ok",
      label: ":5174",
    });
  });

  test("bad при Exited ≠ 0", () => {
    const processes = [proc({ id: 2, project: "100words", status: { Exited: 1 } })];
    expect(projectRowStatus({ name: "100words" }, processes)).toEqual({
      tone: "bad",
      label: "Exited 1",
    });
  });
});

describe("processForProject", () => {
  test("находит по имени", () => {
    const processes = [proc({ id: 1, project: "www", status: "Running" })];
    expect(processForProject(processes, "www")?.id).toBe(1);
    expect(processForProject(processes, "other")).toBeUndefined();
  });
});

describe("buildDashboardSummary", () => {
  test("считает проекты, падения, конфликты и задачи", () => {
    const projects = [project("a", "/a", [project("b", "/b")])];
    const processes = [
      proc({ id: 1, project: "a", status: "Running" }),
      proc({ id: 2, project: "b", status: { Exited: 1 } }),
      proc({ id: 3, project: "c", status: "Crashed" }),
    ];
    const ports = [
      port({ port: 5173, pid: 1, status: "conflict" }),
      port({ port: 5174, pid: 2, status: "ok" }),
    ];
    expect(
      buildDashboardSummary({
        projects,
        processes,
        ports,
        tasks: [{}, {}, {}],
      }),
    ).toEqual({
      projectCount: 2,
      crashedCount: 2,
      portConflictCount: 1,
      taskCount: 3,
    });
  });
});

describe("formatUpdatedAt", () => {
  test("HH:MM по ru-RU", () => {
    const text = formatUpdatedAt(new Date("2026-10-31T12:04:00"));
    expect(text).toMatch(/12:04/);
  });
});
