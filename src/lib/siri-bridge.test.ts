import { describe, expect, test } from "bun:test";
import {
  APPLICATION_NAME_PLACEHOLDER,
  SIRI_ASK_PHRASES,
  SIRI_PRODUCTION_INTENTS,
  SIRI_RESTART_PHRASES,
  buildPulseSockRequest,
  buildRestartSockRequest,
  formatPulseDialog,
  formatRestartDialog,
  parseSockEnvelope,
  phraseIncludesApplicationName,
  productionIntents,
  pulseFromSockOutput,
  restartFromSockOutput,
  restartRequiresConfirmation,
  spikeVerdict,
} from "./siri-bridge";

describe("SIRI_ASK_PHRASES", () => {
  test("every phrase includes applicationName placeholder", () => {
    expect(SIRI_ASK_PHRASES.length).toBeGreaterThanOrEqual(3);
    for (const p of SIRI_ASK_PHRASES) {
      expect(phraseIncludesApplicationName(p.swift)).toBe(true);
      expect(p.swift).toContain(APPLICATION_NAME_PLACEHOLDER);
    }
    expect(SIRI_ASK_PHRASES.some((p) => p.exampleRu.toLowerCase().includes("demerzel"))).toBe(
      true,
    );
  });

  test("russian examples mention Demerzel and project", () => {
    const ru = SIRI_ASK_PHRASES.filter((p) => p.locale === "ru");
    expect(ru.some((p) => p.exampleRu.includes("Спроси Demerzel"))).toBe(true);
    expect(
      ru.every((p) => /parser|проект/i.test(p.exampleRu) || p.exampleRu.includes("parser")),
    ).toBe(true);
  });
});

describe("SIRI_RESTART_PHRASES", () => {
  test("every restart phrase includes applicationName", () => {
    expect(SIRI_RESTART_PHRASES.length).toBeGreaterThanOrEqual(2);
    for (const p of SIRI_RESTART_PHRASES) {
      expect(phraseIncludesApplicationName(p.swift)).toBe(true);
      expect(p.exampleRu.toLowerCase()).toContain("demerzel");
    }
  });

  test("ru restart example", () => {
    expect(
      SIRI_RESTART_PHRASES.some((p) => p.exampleRu === "Перезапусти Demerzel parser"),
    ).toBe(true);
  });
});

describe("formatPulseDialog", () => {
  test("matches spike dialog line", () => {
    expect(
      formatPulseDialog({
        project: "parser",
        summary: "1500 новых лемм, 450 проверено",
        eta_seconds: 600,
        state: "running",
      }),
    ).toBe("parser: 1500 новых лемм, 450 проверено, ещё 10 мин");
  });

  test("without eta", () => {
    expect(formatPulseDialog({ summary: "сборка" }, "web")).toBe("web: сборка");
  });
});

describe("formatRestartDialog", () => {
  test("restarted status", () => {
    expect(formatRestartDialog({ project: "parser", status: "restarted" })).toBe(
      "parser: restarted",
    );
  });
});

describe("dem.sock request/envelope", () => {
  test("pulse request", () => {
    expect(buildPulseSockRequest({ root: "/eco", project: "parser" })).toEqual({
      action: "pulse",
      root: "/eco",
      project: "parser",
      json: true,
    });
  });

  test("restart request for #101", () => {
    expect(buildRestartSockRequest({ root: "/eco", project: "parser" })).toEqual({
      action: "restart",
      root: "/eco",
      project: "parser",
      json: true,
    });
  });

  test("parse envelope and pulse output", () => {
    const pulse = {
      project: "parser",
      summary: "1500 новых лемм, 450 проверено",
      eta_seconds: 600,
      state: "running",
    };
    const env = parseSockEnvelope(
      JSON.stringify({ output: JSON.stringify(pulse), error: null }),
    );
    expect(env.error).toBeNull();
    expect(formatPulseDialog(pulseFromSockOutput(env.output))).toBe(
      "parser: 1500 новых лемм, 450 проверено, ещё 10 мин",
    );
  });

  test("parse restart output", () => {
    const restart = { project: "parser", status: "restarted", id: 7 };
    expect(formatRestartDialog(restartFromSockOutput(JSON.stringify(restart)))).toBe(
      "parser: restarted",
    );
  });

  test("envelope with error", () => {
    const env = parseSockEnvelope(
      JSON.stringify({ output: "", error: "missing field `root`" }),
    );
    expect(env.error).toBe("missing field `root`");
  });
});

describe("productionIntents #101", () => {
  test("AskDemerzelStatus + RestartProcess via dem.sock appex", () => {
    expect(SIRI_PRODUCTION_INTENTS).toContain("AskDemerzelStatus");
    expect(SIRI_PRODUCTION_INTENTS).toContain("RestartProcess");
    expect(productionIntents()).toEqual({
      status: "AskDemerzelStatus",
      restart: "RestartProcess",
      transport: "unix-socket",
      appex: "DemerzelAppIntents.appex",
      confirmation: true,
    });
    expect(restartRequiresConfirmation()).toBe(true);
  });
});

describe("spikeVerdict", () => {
  test("go → #101 via unix-socket", () => {
    expect(spikeVerdict()).toEqual({
      status: "go",
      transport: "unix-socket",
      nextIssue: 101,
    });
  });
});
