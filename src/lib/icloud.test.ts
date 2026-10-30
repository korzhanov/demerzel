import { describe, expect, test } from "bun:test";
import {
  DEFAULT_ICLOUD_FOLDER,
  isICloudSyncEnabled,
  relativeSummaryPath,
  sanitizeICloudFolder,
  SUMMARY_FILE,
} from "./icloud";

describe("sanitizeICloudFolder", () => {
  test("defaults empty and whitespace", () => {
    expect(sanitizeICloudFolder("")).toBe(DEFAULT_ICLOUD_FOLDER);
    expect(sanitizeICloudFolder("  ")).toBe(DEFAULT_ICLOUD_FOLDER);
  });

  test("rejects parent and absolute", () => {
    expect(sanitizeICloudFolder("../evil")).toBe(DEFAULT_ICLOUD_FOLDER);
    expect(sanitizeICloudFolder("/abs")).toBe(DEFAULT_ICLOUD_FOLDER);
    expect(sanitizeICloudFolder("a/../../b")).toBe(DEFAULT_ICLOUD_FOLDER);
  });

  test("keeps nested safe path", () => {
    expect(sanitizeICloudFolder("Demerzel/Status")).toBe("Demerzel/Status");
    expect(sanitizeICloudFolder("Demerzel/./x")).toBe("Demerzel/x");
  });
});

describe("relativeSummaryPath", () => {
  test("joins folder and file", () => {
    expect(relativeSummaryPath()).toBe(`${DEFAULT_ICLOUD_FOLDER}/${SUMMARY_FILE}`);
    expect(relativeSummaryPath("Demerzel/Status")).toBe(
      `Demerzel/Status/${SUMMARY_FILE}`,
    );
  });
});

describe("isICloudSyncEnabled", () => {
  test("false by default", () => {
    expect(isICloudSyncEnabled()).toBe(false);
    expect(isICloudSyncEnabled({})).toBe(false);
    expect(isICloudSyncEnabled({ enabled: false })).toBe(false);
  });

  test("true only when enabled", () => {
    expect(isICloudSyncEnabled({ enabled: true })).toBe(true);
  });
});
