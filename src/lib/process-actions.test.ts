import { describe, expect, test } from "bun:test";
import {
  attachablePorts,
  localhostUrl,
  logsUnavailableHint,
  processActions,
  processStateLabel,
} from "./process-actions";
import type { PortRow } from "./types";

describe("processActions", () => {
  test("running process can pause, stop, restart and open its port", () => {
    expect(processActions({ status: "Running", paused: false, port: 5173 })).toEqual({
      start: false,
      pause: true,
      resume: false,
      stop: true,
      restart: true,
      open: true,
    });
  });

  test("paused process offers resume instead of pause", () => {
    const a = processActions({ status: "Running", paused: true, port: null });
    expect(a.pause).toBe(false);
    expect(a.resume).toBe(true);
    expect(a.open).toBe(false);
  });

  test("stopped process can only start or restart", () => {
    const a = processActions({ status: "Stopped", paused: false, port: 5173 });
    expect(a.start).toBe(true);
    expect(a.stop).toBe(false);
    expect(a.pause).toBe(false);
    expect(a.open).toBe(false);
  });
});

describe("processStateLabel and origin hints", () => {
  test("state labels", () => {
    expect(processStateLabel({ status: "Running", paused: true })).toBe("На паузе");
    expect(processStateLabel({ status: "Running", paused: false })).toBe("Работает");
    expect(processStateLabel({ status: "Stopped", paused: false })).toBe("Остановлен");
    expect(processStateLabel({ status: { Exited: 1 } as never, paused: false })).toBe("Завершён, код 1");
  });

  test("attached process explains missing logs", () => {
    expect(logsUnavailableHint("attached")).toContain("присоединённого");
    expect(logsUnavailableHint("started")).toBeNull();
    expect(localhostUrl(5173)).toBe("http://localhost:5173");
  });

  test("only unmanaged, recognized, non-conflicting ports can be attached", () => {
    const row = (over: Partial<PortRow>) =>
      ({ port: 1, pid: 1, command: "x", project: "api", status: "ok", managed_id: null, ...over }) as PortRow;
    const ports = [
      row({ port: 5173 }),
      row({ port: 3000, managed_id: 4 }),
      row({ port: 8080, project: null }),
      row({ port: 9000, status: "conflict" }),
    ];
    expect(attachablePorts(ports).map((p) => p.port)).toEqual([5173]);
  });
});
