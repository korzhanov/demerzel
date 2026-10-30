import { describe, expect, test } from "bun:test";
import {
  agentRunStatusLabel,
  canAcceptAgentRun,
  canDiscardAgentRun,
  hasAgentRunReviewPayload,
  manualMergeCommand,
  shellSingleQuote,
} from "./agents";
import {
  AGENT_RUN_STATUSES,
  DEFAULT_AGENT_CONCURRENCY,
  DEFAULT_MIN_FREE_MEMORY_MB,
  DEFAULT_OLLAMA_RESERVE_MB,
  agentsConfigOrDefault,
  canTransitionAgentRun,
  requiredFreeMemoryMb,
  statusAfterTests,
  type AgentRunRecord,
} from "./types";

describe("agents config defaults (#29)", () => {
  test("defaults match Rust AgentsConfig", () => {
    expect(DEFAULT_AGENT_CONCURRENCY).toBe(1);
    expect(DEFAULT_MIN_FREE_MEMORY_MB).toBe(2048);
    expect(DEFAULT_OLLAMA_RESERVE_MB).toBe(4096);
    expect(agentsConfigOrDefault(undefined)).toEqual({
      max_concurrent: 1,
      min_free_memory_mb: 2048,
      ollama_reserve_mb: 4096,
    });
  });

  test("partial override keeps other defaults", () => {
    expect(agentsConfigOrDefault({ max_concurrent: 2 })).toEqual({
      max_concurrent: 2,
      min_free_memory_mb: 2048,
      ollama_reserve_mb: 4096,
    });
  });

  test("ollama reserve raises required free memory", () => {
    expect(requiredFreeMemoryMb(undefined, false)).toBe(2048);
    expect(requiredFreeMemoryMb(undefined, true)).toBe(2048 + 4096);
    expect(
      requiredFreeMemoryMb(
        { min_free_memory_mb: 1024, ollama_reserve_mb: 512 },
        true,
      ),
    ).toBe(1536);
  });
});

describe("agent run status + tests (#31)", () => {
  test("status set is snake_case and complete", () => {
    expect([...AGENT_RUN_STATUSES]).toEqual([
      "queued",
      "running",
      "needs_review",
      "failed",
      "discarded",
      "accepted",
    ]);
  });

  test("cannot accept without review", () => {
    expect(canTransitionAgentRun("running", "accepted")).toBe(false);
    expect(canTransitionAgentRun("needs_review", "accepted")).toBe(true);
    expect(canTransitionAgentRun("running", "needs_review")).toBe(true);
    expect(canTransitionAgentRun("running", "failed")).toBe(true);
  });

  test("statusAfterTests mirrors Rust", () => {
    expect(statusAfterTests({ skipped: true, exitCode: null })).toBe(
      "needs_review",
    );
    expect(statusAfterTests({ skipped: false, exitCode: 0 })).toBe(
      "needs_review",
    );
    expect(statusAfterTests({ skipped: false, exitCode: 1 })).toBe("failed");
  });
});

describe("agent review UI helpers (#32)", () => {
  test("status labels and accept/discard gates", () => {
    expect(agentRunStatusLabel("needs_review")).toBe("на ревью");
    expect(canAcceptAgentRun("needs_review")).toBe(true);
    expect(canAcceptAgentRun("failed")).toBe(false);
    expect(canDiscardAgentRun("needs_review")).toBe(true);
    expect(canDiscardAgentRun("failed")).toBe(true);
    expect(canDiscardAgentRun("accepted")).toBe(false);
  });

  test("manualMergeCommand mirrors Rust quoting", () => {
    expect(shellSingleQuote("demo's")).toBe(`'demo'\\''s'`);
    expect(manualMergeCommand("/tmp/demo's", "run-1")).toBe(
      "git -C '/tmp/demo'\\''s' merge --no-ff agent/run-1",
    );
  });

  test("hasAgentRunReviewPayload", () => {
    const base: Pick<AgentRunRecord, "diff" | "tests"> = {
      diff: null,
      tests: null,
    };
    expect(hasAgentRunReviewPayload(base)).toBe(false);
    expect(hasAgentRunReviewPayload({ ...base, diff: "x" })).toBe(true);
    expect(
      hasAgentRunReviewPayload({
        diff: null,
        tests: {
          command: "t",
          exit_code: 0,
          log: "ok",
          skipped: false,
          duration_ms: 1,
        },
      }),
    ).toBe(true);
  });
});
