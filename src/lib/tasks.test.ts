/// <reference types="bun-types" />
import { describe, expect, test } from "bun:test";
import {
  TASK_BOARD_COLUMNS,
  TASK_COLUMN_PREVIEW,
  canOpenTask,
  canStartAgentFromTask,
  countByStatus,
  boardRootPath,
  filterTasksByProject,
  isInProjectScope,
  formatTaskBoardSummary,
  splitColumnPreview,
  taskStatusLabel,
  tasksForColumn,
} from "./tasks";
import type { Task } from "./types";

function task(partial: Partial<Task> & Pick<Task, "status">): Task {
  return {
    id: partial.id ?? "1",
    project: partial.project ?? "/code/demo",
    title: partial.title ?? "t",
    status: partial.status,
    source: partial.source ?? "todo",
    url: partial.url === undefined ? null : partial.url,
  };
}

describe("taskStatusLabel", () => {
  test("keeps wire statuses stable for UI and adapters", () => {
    expect(taskStatusLabel("todo")).toBe("todo");
    expect(taskStatusLabel("in_progress")).toBe("in_progress");
    expect(taskStatusLabel("done")).toBe("done");
    expect(taskStatusLabel("blocked")).toBe("blocked");
  });
});

describe("TASK_BOARD_COLUMNS", () => {
  test("covers all normalized statuses in wireframe order", () => {
    expect(TASK_BOARD_COLUMNS.map((c) => c.status)).toEqual([
      "todo",
      "in_progress",
      "done",
      "blocked",
    ]);
    expect(TASK_BOARD_COLUMNS.map((c) => c.label)).toEqual([
      "К выполнению",
      "В работе",
      "Готово",
      "Заблокировано",
    ]);
  });
});

describe("filterTasksByProject", () => {
  test("empty filter keeps all projects; path filter narrows", () => {
    const rows = [
      task({ id: "a", project: "/a", status: "todo" }),
      task({ id: "b", project: "/b", status: "done" }),
    ];
    expect(filterTasksByProject(rows, "").map((t) => t.id)).toEqual(["a", "b"]);
    expect(filterTasksByProject(rows, "/b").map((t) => t.id)).toEqual(["b"]);
  });
});

describe("project board scope", () => {
  const sub = { name: "www", path: "/a/app/www", children: [] };
  const root = { name: "app", path: "/a/app", children: [sub] };
  const other = { name: "api", path: "/a/api", children: [] };

  test("errors are shown only for projects inside the board scope", () => {
    expect(isInProjectScope("/a/app/www", "")).toBe(true);
    expect(isInProjectScope("/a/app/www", "/a/app")).toBe(true);
    expect(isInProjectScope("/a/app", "/a/app")).toBe(true);
    expect(isInProjectScope("/a/api", "/a/app")).toBe(false);
    expect(isInProjectScope("/a/app2", "/a/app")).toBe(false);
  });

  test("subproject resolves to its top-level project", () => {
    expect(boardRootPath([root, other], "/a/app/www")).toBe("/a/app");
    expect(boardRootPath([root, other], "/a/api")).toBe("/a/api");
    expect(boardRootPath([root, other], "/nope")).toBe("/nope");
    expect(boardRootPath([root], "")).toBe("");
  });

  test("project filter includes subprojects but not look-alike paths", () => {
    const rows = [
      task({ id: "root", project: "/a/app", status: "todo" }),
      task({ id: "sub", project: "/a/app/www", status: "todo" }),
      task({ id: "twin", project: "/a/app2", status: "todo" }),
    ];
    expect(filterTasksByProject(rows, "/a/app").map((t) => t.id)).toEqual(["root", "sub"]);
    expect(filterTasksByProject(rows, "/a/app/www").map((t) => t.id)).toEqual(["sub"]);
  });
});

describe("tasksForColumn", () => {
  test("returns only matching status", () => {
    const rows = [
      task({ id: "1", status: "todo" }),
      task({ id: "2", status: "in_progress" }),
      task({ id: "3", status: "todo" }),
    ];
    expect(tasksForColumn(rows, "todo").map((t) => t.id)).toEqual(["1", "3"]);
    expect(tasksForColumn(rows, "blocked")).toEqual([]);
  });
});

describe("splitColumnPreview", () => {
  test("keeps short lists intact; spoils the rest past preview limit", () => {
    expect(TASK_COLUMN_PREVIEW).toBe(3);
    const ids = ["a", "b", "c", "d", "e"];
    expect(splitColumnPreview(ids, 3)).toEqual({
      preview: ["a", "b", "c"],
      rest: ["d", "e"],
    });
    expect(splitColumnPreview(["x", "y"], 3)).toEqual({
      preview: ["x", "y"],
      rest: [],
    });
    expect(splitColumnPreview(ids, 0)).toEqual({ preview: [], rest: ids });
  });
});

describe("canOpenTask", () => {
  test("allows http(s) issue URLs and absolute file paths", () => {
    expect(canOpenTask({ url: "https://github.com/o/r/issues/1" })).toBe(true);
    expect(canOpenTask({ url: "http://example.com/x" })).toBe(true);
    expect(canOpenTask({ url: "/code/demo/TODO.md" })).toBe(true);
    expect(canOpenTask({ url: null })).toBe(false);
    expect(canOpenTask({ url: "" })).toBe(false);
    expect(canOpenTask({ url: "relative/TODO.md" })).toBe(false);
  });
});

describe("canStartAgentFromTask", () => {
  test("allows all statuses except done", () => {
    expect(canStartAgentFromTask({ status: "todo" })).toBe(true);
    expect(canStartAgentFromTask({ status: "in_progress" })).toBe(true);
    expect(canStartAgentFromTask({ status: "blocked" })).toBe(true);
    expect(canStartAgentFromTask({ status: "done" })).toBe(false);
  });
});

describe("countByStatus", () => {
  test("counts each normalized status", () => {
    expect(
      countByStatus([
        task({ status: "todo" }),
        task({ status: "in_progress" }),
        task({ status: "done" }),
        task({ status: "done" }),
        task({ status: "blocked" }),
      ]),
    ).toEqual({ todo: 1, in_progress: 1, done: 2, blocked: 1 });
  });
});

describe("formatTaskBoardSummary", () => {
  test("matches mockup strip; omits blocked when zero", () => {
    expect(
      formatTaskBoardSummary({ todo: 5, in_progress: 2, done: 9, blocked: 0 }),
    ).toBe("В работе 2 · Todo 5 · Готово 9");
    expect(
      formatTaskBoardSummary({ todo: 1, in_progress: 0, done: 0, blocked: 3 }),
    ).toBe("В работе 0 · Todo 1 · Готово 0 · Заблок. 3");
  });
});
