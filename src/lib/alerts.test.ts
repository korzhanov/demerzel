/// <reference types="bun-types" />
import { describe, expect, test } from "bun:test";
import { severityLabel, shouldNotify, shouldSpeak } from "./alerts";
import type { AlertSeverity } from "./alerts";

describe("shouldSpeak", () => {
  test("only critical speaks", () => {
    expect(shouldSpeak("critical")).toBe(true);
    expect(shouldSpeak("important")).toBe(false);
    expect(shouldSpeak("info")).toBe(false);
  });
});

describe("shouldNotify", () => {
  test("important and critical notify; info is UI-only", () => {
    expect(shouldNotify("critical")).toBe(true);
    expect(shouldNotify("important")).toBe(true);
    expect(shouldNotify("info")).toBe(false);
  });
});

describe("severityLabel", () => {
  test("stable RU labels for wire severities", () => {
    const cases: [AlertSeverity, string][] = [
      ["critical", "Критично"],
      ["important", "Важно"],
      ["info", "Инфо"],
    ];
    for (const [sev, label] of cases) {
      expect(severityLabel(sev)).toBe(label);
    }
  });
});
