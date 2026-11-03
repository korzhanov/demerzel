import { describe, expect, test } from "bun:test";
import {
  agentInstallHint,
  agentRunnerLabel,
  agentRunnerOptionLabel,
  agentRunStatusLabel,
  autofixModeLabel,
  canAcceptAgentRun,
  canDiscardAgentRun,
  hasAgentRunReviewPayload,
  manualMergeCommand,
  shellSingleQuote,
} from "./agents";
import {
  AGENT_RUN_STATUSES,
  AGENT_RUNNERS,
  AUTOFIX_MODES,
  DEFAULT_AGENT_CONCURRENCY,
  DEFAULT_AGENT_RUNNER,
  DEFAULT_AUTOFIX_MODE,
  DEFAULT_MIN_FREE_MEMORY_MB,
  DEFAULT_OLLAMA_MODEL,
  DEFAULT_OLLAMA_RESERVE_MB,
  agentsConfigOrDefault,
  canTransitionAgentRun,
  parseAgentRunner,
  parseAutofixMode,
  requiredFreeMemoryMb,
  statusAfterTests,
  type AgentCatalogEntry,
  type AgentRunRecord,
} from "./types";

describe("agents config defaults (#29 / #30 / #95)", () => {
  test("defaults match Rust AgentsConfig", () => {
    expect(DEFAULT_AGENT_CONCURRENCY).toBe(1);
    expect(DEFAULT_MIN_FREE_MEMORY_MB).toBe(2048);
    expect(DEFAULT_OLLAMA_RESERVE_MB).toBe(4096);
    expect(DEFAULT_AGENT_RUNNER).toBe("claude");
    expect(DEFAULT_OLLAMA_MODEL).toBe("qwen2.5-coder:7b");
    expect([...AGENT_RUNNERS]).toEqual([
      "claude",
      "codex",
      "gemini",
      "aider",
      "opencode",
      "cursor_agent",
      "hermes",
      "grok",
      "ollama",
    ]);
    expect(DEFAULT_AUTOFIX_MODE).toBe("off");
    expect([...AUTOFIX_MODES]).toEqual(["off", "suggest", "auto"]);
    expect(agentsConfigOrDefault(undefined)).toEqual({
      max_concurrent: 1,
      min_free_memory_mb: 2048,
      ollama_reserve_mb: 4096,
      runner: "claude",
      ollama_model: "qwen2.5-coder:7b",
      acp_command: "",
      autofix: "off",
      catalog: [],
    });
  });

  test("partial override keeps other defaults", () => {
    expect(agentsConfigOrDefault({ max_concurrent: 2 })).toEqual({
      max_concurrent: 2,
      min_free_memory_mb: 2048,
      ollama_reserve_mb: 4096,
      runner: "claude",
      ollama_model: "qwen2.5-coder:7b",
      acp_command: "",
      autofix: "off",
      catalog: [],
    });
  });

  test("autofix mode parse and labels (#96)", () => {
    expect(parseAutofixMode("suggest")).toBe("suggest");
    expect(parseAutofixMode("auto")).toBe("auto");
    expect(parseAutofixMode("nope")).toBe("off");
    expect(autofixModeLabel("off")).toBe("Автофикс выкл");
    expect(autofixModeLabel("suggest")).toBe("Предлагать фикс");
    expect(autofixModeLabel("auto")).toBe("Автофикс");
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

  test("parseAgentRunner and labels (#30 / #95)", () => {
    expect(parseAgentRunner("ollama")).toBe("ollama");
    expect(parseAgentRunner("Claude")).toBe("claude");
    expect(parseAgentRunner("cursor-agent")).toBe("cursor_agent");
    expect(parseAgentRunner("codex")).toBe("codex");
    expect(parseAgentRunner(undefined)).toBe("claude");
    expect(agentRunnerLabel("ollama")).toBe("Ollama (локально)");
    expect(agentRunnerLabel("cursor_agent")).toBe("Cursor Agent");
    expect(agentRunnerLabel(undefined)).toBe("Claude CLI");
  });

  test("catalog option labels and install hints (#95)", () => {
    const catalog: AgentCatalogEntry[] = [
      {
        runner: "claude",
        binary: "claude",
        available: true,
        version: "1.0.0",
        install_hint: "install claude",
        headless_supported: true,
      },
      {
        runner: "gemini",
        binary: "gemini",
        available: false,
        install_hint: "Установите Gemini CLI",
        headless_supported: false,
      },
      {
        runner: "aider",
        binary: "aider",
        available: true,
        version: "0.9",
        install_hint: "pipx install aider-chat",
        headless_supported: false,
      },
    ];
    expect(agentRunnerOptionLabel("claude", catalog)).toContain("1.0.0");
    expect(agentRunnerOptionLabel("gemini", catalog)).toContain("не установлен");
    expect(agentRunnerOptionLabel("aider", catalog)).toContain("только детект");
    expect(agentInstallHint("gemini", catalog)).toContain("Gemini");
    expect(agentInstallHint("aider", catalog)).toContain("Claude и Ollama");
    expect(agentInstallHint("aider", catalog)).not.toContain("pipx");
    expect(agentInstallHint("claude", catalog)).toBeNull();
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
