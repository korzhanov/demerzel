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
  const www = { name: "www", path: "/work/shop/www" };
  const words = { name: "100words", path: "/work/100words" };

  test("idle без процесса", () => {
    expect(projectRowStatus(www, [])).toEqual({
      tone: "idle",
      label: "нет процесса",
    });
  });

  test("ok с портом", () => {
    const processes = [proc({ id: 1, project: "www", path: www.path, status: "Running", port: 5174 })];
    expect(projectRowStatus(www, processes)).toEqual({
      tone: "ok",
      label: ":5174",
    });
  });

  test("bad при Exited ≠ 0", () => {
    const processes = [proc({ id: 2, project: "100words", path: words.path, status: { Exited: 1 } })];
    expect(projectRowStatus(words, processes)).toEqual({
      tone: "bad",
      label: "Exited 1",
    });
  });

  test("подпроект с тем же именем не получает чужой процесс", () => {
    // Два подпроекта называются www: запущен только один, из папки shop.
    const processes = [proc({ id: 3, project: "www", path: "/work/shop/www", status: "Running", port: 5174 })];
    const other = { name: "www", path: "/work/blog/www" };
    expect(projectRowStatus(other, processes)).toEqual({ tone: "idle", label: "нет процесса" });
    expect(projectRowStatus(www, processes).tone).toBe("ok");
  });
});

describe("processForProject", () => {
  test("находит по папке, а не по имени", () => {
    const processes = [proc({ id: 1, project: "www", path: "/work/shop/www", status: "Running" })];
    expect(processForProject(processes, { path: "/work/shop/www" })?.id).toBe(1);
    expect(processForProject(processes, { path: "/work/blog/www" })).toBeUndefined();
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
