/// <reference types="bun-types" />
import { describe, expect, test } from "bun:test";
import { countByStatus, taskStatusLabel } from "./tasks";
import type { Task } from "./types";

function task(status: Task["status"]): Task {
  return {
    id: "1",
    project: "demo",
    title: "t",
    status,
    source: "test",
    url: null,
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

describe("countByStatus", () => {
  test("counts each normalized status", () => {
    expect(
      countByStatus([
        task("todo"),
        task("in_progress"),
        task("done"),
        task("done"),
        task("blocked"),
      ]),
    ).toEqual({ todo: 1, in_progress: 1, done: 2, blocked: 1 });
  });
});
