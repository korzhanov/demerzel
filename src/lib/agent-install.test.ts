import { describe, expect, test } from "bun:test";
import {
  buildInstallCommand,
  loadAgentInstallTable,
  mergeCatalogPrefs,
  pickPreferredRunner,
  reorderCatalog,
  setCatalogEnabled,
} from "./agent-install";
import type { AgentRunner } from "./types";
import { AGENT_RUNNERS } from "./types";

describe("agent-install table (#140)", () => {
  test("table loads with Terminal.app default and known runners", () => {
    const t = loadAgentInstallTable();
    expect(t.terminal_default).toBe("Terminal.app");
    expect(t.agents.some((a) => a.runner === "cursor_agent" && a.binary === "agent")).toBe(
      true,
    );
    expect(t.agents.find((a) => a.runner === "grok")?.verified).toBe(false);
    expect(t.agents.find((a) => a.runner === "gemini")?.install_hint).toContain(
      "Antigravity",
    );
  });

  test("buildInstallCommand", () => {
    expect(buildInstallCommand("codex")).toContain("brew install");
    expect(buildInstallCommand("hermes")).toContain("hermes-agent");
    expect(buildInstallCommand("claude")).toBeNull();
  });

  test("merge / reorder / enabled / pickPreferred", () => {
    const runners = AGENT_RUNNERS as readonly AgentRunner[];
    let prefs = mergeCatalogPrefs(
      [
        { runner: "codex", enabled: true },
        { runner: "claude", enabled: false },
      ],
      runners,
    );
    expect(prefs[0]?.runner).toBe("codex");
    expect(prefs.find((p) => p.runner === "claude")?.enabled).toBe(false);

    prefs = reorderCatalog(prefs, 0, 2);
    expect(prefs[2]?.runner).toBe("codex");

    prefs = setCatalogEnabled(prefs, "claude", true);
    const pick = pickPreferredRunner(prefs, new Set<AgentRunner>(["claude"]));
    expect(pick).toBe("claude");

    const none = pickPreferredRunner(
      [{ runner: "codex", enabled: false }],
      new Set<AgentRunner>(["codex"]),
    );
    expect(none).toBeNull();
  });
});
