import { describe, expect, test } from "bun:test";
import {
  formatEtaRu,
  formatPulseLine,
  parsePulsePayload,
  projectsWithPulse,
  pulseFingerprint,
  type ProjectWithPulse,
} from "./pulse";

describe("formatEtaRu", () => {
  test("seconds minutes hours", () => {
    expect(formatEtaRu(45)).toBe("ещё 45 с");
    expect(formatEtaRu(600)).toBe("ещё 10 мин");
    expect(formatEtaRu(3600)).toBe("ещё 1 ч");
  });
});

describe("formatPulseLine", () => {
  test("with and without eta", () => {
    expect(
      formatPulseLine("parser", {
        summary: "1500 новых лемм, 450 проверено",
        eta_seconds: 600,
      }),
    ).toBe("parser: 1500 новых лемм, 450 проверено, ещё 10 мин");
    expect(formatPulseLine("web", { summary: "сборка" })).toBe("web: сборка");
  });
});

describe("projectsWithPulse / pulseFingerprint", () => {
  const projects: ProjectWithPulse[] = [
    {
      name: "api",
      pulse: {
        summary: "1500 новых лемм, 450 проверено",
        eta_seconds: 600,
      },
    },
    { name: "web", pulse: { summary: "  " } },
    { name: "idle" },
  ];

  test("skips empty pulse", () => {
    expect(projectsWithPulse(projects).map((p) => p.name)).toEqual(["api"]);
  });

  test("fingerprint ignores empty and is stable", () => {
    const a = pulseFingerprint(projects);
    const b = pulseFingerprint([
      ...projects,
      { name: "x", pulse: null },
    ]);
    expect(a).toBe(b);
    expect(a).toContain("ещё 10 мин");
    expect(
      pulseFingerprint([
        {
          name: "api",
          pulse: { summary: "другое", eta_seconds: 600 },
        },
      ]),
    ).not.toBe(a);
  });
});

describe("parsePulsePayload (#97)", () => {
  test("accepts metrics and eta", () => {
    const p = parsePulsePayload({
      summary: "1500 новых лемм, 450 проверено",
      metrics: { parsed: 1500, checked: 450 },
      eta_seconds: 600,
      state: "running",
    });
    expect(p.summary).toBe("1500 новых лемм, 450 проверено");
    expect(p.metrics).toEqual({ parsed: 1500, checked: 450 });
    expect(p.eta_seconds).toBe(600);
  });

  test("rejects empty summary", () => {
    expect(() => parsePulsePayload({ summary: "  " })).toThrow(/summary/);
  });
});
