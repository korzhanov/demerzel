/// <reference types="bun-types" />
import { describe, expect, test } from "bun:test";
import {
  deliveryFlags,
  isQuietHours,
  isQuietHoursLocal,
  notifyTitle,
  parseHhMmToMinutes,
  processIdForProject,
  severityLabel,
  shouldDeliverNotify,
  shouldNotify,
  shouldSpeak,
} from "./alerts";
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

describe("quiet hours", () => {
  test("parse HH:MM", () => {
    expect(parseHhMmToMinutes("23:00")).toBe(23 * 60);
    expect(parseHhMmToMinutes("08:00")).toBe(8 * 60);
    expect(parseHhMmToMinutes("25:00")).toBeNull();
  });

  test("cross-midnight window 23:00–08:00", () => {
    const start = 23 * 60;
    const end = 8 * 60;
    expect(isQuietHoursLocal(23 * 60 + 30, start, end)).toBe(true);
    expect(isQuietHoursLocal(3 * 60, start, end)).toBe(true);
    expect(isQuietHoursLocal(8 * 60, start, end)).toBe(false);
    expect(isQuietHoursLocal(12 * 60, start, end)).toBe(false);
  });

  test("defaults and disabled flag", () => {
    expect(isQuietHours(23 * 60 + 15)).toBe(true);
    expect(isQuietHours(12 * 60)).toBe(false);
    expect(isQuietHours(23 * 60, { enabled: false })).toBe(false);
  });
});

describe("deliveryFlags", () => {
  test("quiet hours mute voice; notify only critical", () => {
    expect(deliveryFlags("critical", { quiet: true })).toEqual({
      speak: false,
      notify: true,
    });
    expect(deliveryFlags("important", { quiet: true })).toEqual({
      speak: false,
      notify: false,
    });
  });

  test("voice throttle clears speak only", () => {
    expect(
      deliveryFlags("critical", { quiet: false, voiceThrottled: true }),
    ).toEqual({ speak: false, notify: true });
  });

  test("daytime critical keeps both", () => {
    expect(deliveryFlags("critical", { quiet: false })).toEqual({
      speak: true,
      notify: true,
    });
  });
});

describe("notify delivery (#22)", () => {
  test("titles by severity", () => {
    expect(notifyTitle("critical")).toBe("Demerzel · Критично");
    expect(notifyTitle("important")).toBe("Demerzel · Важно");
    expect(notifyTitle("info")).toBe("Demerzel");
  });

  test("shouldDeliverNotify follows gate flag only", () => {
    expect(shouldDeliverNotify({ notify: true })).toBe(true);
    expect(shouldDeliverNotify({ notify: false })).toBe(false);
    expect(
      shouldDeliverNotify(deliveryFlags("important", { quiet: true })),
    ).toBe(false);
    expect(
      shouldDeliverNotify(deliveryFlags("critical", { quiet: true })),
    ).toBe(true);
  });

  test("processIdForProject picks running row by name", () => {
    const rows = [
      { id: 1, project: "api" },
      { id: 2, project: "web" },
    ];
    expect(processIdForProject(rows, "web")).toBe(2);
    expect(processIdForProject(rows, "missing")).toBeNull();
  });
});
