import { describe, expect, test } from "bun:test";
import { latestRunForTask, runProgress } from "./run-progress";
import type { AgentRunRecord } from "./types";

function run(partial: Partial<AgentRunRecord> & Pick<AgentRunRecord, "id" | "status">): AgentRunRecord {
  return {
    project: "p",
    project_path: "/p",
    task_id: null,
    worktree: null,
    base_ref: "HEAD",
    tests: null,
    diff: null,
    started_at: null,
    finished_at: null,
    ...partial,
  } as AgentRunRecord;
}

describe("runProgress", () => {
  test("queued and running are not finished", () => {
    expect(runProgress({ status: "queued" }).percent).toBeLessThan(100);
    const running = runProgress({ status: "running" });
    expect(running.indeterminate).toBe(true);
    expect(running.percent).toBeLessThan(100);
  });

  test("terminal states reach 100 with their tone", () => {
    expect(runProgress({ status: "needs_review" })).toMatchObject({ percent: 100, tone: "done" });
    expect(runProgress({ status: "failed" })).toMatchObject({ percent: 100, tone: "failed" });
  });
});

describe("latestRunForTask", () => {
  test("picks the most recent run for the task", () => {
    const runs = [
      run({ id: "a", status: "failed", task_id: "t1", started_at: "2026-11-01T10:00:00Z" }),
      run({ id: "b", status: "running", task_id: "t1", started_at: "2026-11-02T09:00:00Z" }),
      run({ id: "c", status: "running", task_id: "t2", started_at: "2026-11-03T09:00:00Z" }),
    ];
    expect(latestRunForTask(runs, "t1")?.id).toBe("b");
    expect(latestRunForTask(runs, "missing")).toBeUndefined();
  });
});
