import { describe, expect, test } from "bun:test";
import {
  absoluteAppGroupSummaryPath,
  APP_GROUP_ID,
  isWidgetKitFallbackEnabled,
  relativeAppGroupSummaryPath,
  SUMMARY_FILE,
  WIDGET_KIND,
} from "./widgetkit";

describe("widgetkit constants", () => {
  test("match ADR / tauri.conf app group", () => {
    expect(APP_GROUP_ID).toBe("group.app.demerzel.orchestrator");
    expect(SUMMARY_FILE).toBe("summary.json");
    expect(WIDGET_KIND).toBe("DemerzelNativeWidget");
  });
});

describe("isWidgetKitFallbackEnabled", () => {
  test("false by default", () => {
    expect(isWidgetKitFallbackEnabled()).toBe(false);
    expect(isWidgetKitFallbackEnabled({})).toBe(false);
    expect(isWidgetKitFallbackEnabled({ enabled: false })).toBe(false);
  });

  test("true only when enabled", () => {
    expect(isWidgetKitFallbackEnabled({ enabled: true })).toBe(true);
  });
});

describe("paths", () => {
  test("relative under app group", () => {
    expect(relativeAppGroupSummaryPath()).toBe(
      `${APP_GROUP_ID}/${SUMMARY_FILE}`,
    );
  });

  test("absolute Group Containers path", () => {
    expect(absoluteAppGroupSummaryPath("/Users/demo")).toBe(
      `/Users/demo/Library/Group Containers/${APP_GROUP_ID}/${SUMMARY_FILE}`,
    );
    expect(absoluteAppGroupSummaryPath("/Users/demo/")).toBe(
      `/Users/demo/Library/Group Containers/${APP_GROUP_ID}/${SUMMARY_FILE}`,
    );
  });
});
