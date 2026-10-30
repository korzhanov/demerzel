import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "../..");

describe("macOS widget embed (#103)", () => {
  test("tauri.conf embeds PlugIns and builds appex before bundle", () => {
    const conf = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"));
    expect(conf.plugins.widgets.appGroup).toBe("group.app.demerzel.orchestrator");
    expect(conf.plugins.widgets.extensionBundleId).toBe(
      "app.demerzel.orchestrator.widgetkit",
    );
    expect(conf.plugins.widgets.transport).toBe("widgetContainer");
    expect(conf.build.beforeBundleCommand).toContain("macos-widget/build-widget.sh");
    expect(conf.bundle.macOS.files["PlugIns/TauriWidgetExtension.appex"]).toContain(
      "TauriWidgetExtension.appex",
    );
    expect(conf.bundle.macOS.entitlements).toContain("macos-widget/App.entitlements");
  });

  test("sidecar sources and scripts exist", () => {
    expect(existsSync(join(root, "src-tauri/macos-widget/Sources/MyWidget.swift"))).toBe(
      true,
    );
    expect(existsSync(join(root, "src-tauri/macos-widget/build-widget.sh"))).toBe(true);
    expect(existsSync(join(root, "src-tauri/macos-widget/project.yml"))).toBe(true);
    expect(existsSync(join(root, "scripts/sign-macos.sh"))).toBe(true);
    expect(existsSync(join(root, "scripts/TauriWidgets.Package.swift"))).toBe(true);
    expect(existsSync(join(root, "widget.json"))).toBe(true);
  });

  test("MyWidget display name is Demerzel", () => {
    const swift = readFileSync(
      join(root, "src-tauri/macos-widget/Sources/MyWidget.swift"),
      "utf8",
    );
    expect(swift).toContain('configurationDisplayName("Demerzel")');
    expect(swift).toContain('let appGroup = "group.app.demerzel.orchestrator"');
    expect(swift).toContain('let widgetId = "default"');
  });
});
