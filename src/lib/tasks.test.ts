/// <reference types="bun-types" />
import { describe, expect, test } from "bun:test";
import {
  TASK_BOARD_COLUMNS,
  canOpenTask,
  countByStatus,
  filterTasksByProject,
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
