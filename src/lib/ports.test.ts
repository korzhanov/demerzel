/// <reference types="bun-types" />
import { describe, expect, test } from "bun:test";
import {
  canStopManagedPort,
  isSystemPort,
  clampPortPollInterval,
  portStatusLabel,
} from "./ports";
import type { PortRow } from "./types";

function row(partial: Partial<PortRow> & Pick<PortRow, "managed_id" | "status">): PortRow {
  return {
    port: 5100,
    project: "api",
    command: "node",
    pid: 42,
    ...partial,
  };
}

describe("portStatusLabel", () => {
  test("maps inventory statuses for the ports table", () => {
    expect(portStatusLabel("ok")).toBe("ok");
    expect(portStatusLabel("conflict")).toBe("конфликт");
    expect(portStatusLabel("foreign")).toBe("чужой");
  });
});

describe("canStopManagedPort", () => {
  test("allows stop only for orchestrator-managed listeners", () => {
    expect(canStopManagedPort(row({ managed_id: 7, status: "ok" }))).toBe(true);
    expect(canStopManagedPort(row({ managed_id: null, status: "foreign" }))).toBe(false);
    expect(canStopManagedPort(row({ managed_id: null, status: "conflict" }))).toBe(false);
  });
});

describe("clampPortPollInterval", () => {
  test("keeps values inside 1..60 seconds", () => {
    expect(clampPortPollInterval(5)).toBe(5);
    expect(clampPortPollInterval(0)).toBe(1);
    expect(clampPortPollInterval(99)).toBe(60);
    expect(clampPortPollInterval(3.9)).toBe(3);
    expect(clampPortPollInterval(Number.NaN)).toBe(5);
  });
});

describe("isSystemPort", () => {
  test("macOS system daemons are system", () => {
    expect(isSystemPort({ command: "rapportd", status: "foreign", managed_id: null })).toBe(true);
  });
  test("IDE helpers are system", () => {
    expect(isSystemPort({ command: "Cursor Helper (Plugin)", status: "foreign", managed_id: null })).toBe(true);
  });
  test("a dev server is not system", () => {
    expect(isSystemPort({ command: "node", status: "foreign", managed_id: null })).toBe(false);
  });
  test("managed or conflicting ports are never system", () => {
    expect(isSystemPort({ command: "rapportd", status: "conflict", managed_id: null })).toBe(false);
    expect(isSystemPort({ command: "rapportd", status: "ok", managed_id: 3 })).toBe(false);
  });
});
