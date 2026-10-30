import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "../..");

describe("macOS Siri App Intents embed (#101)", () => {
  test("tauri.conf builds and embeds DemerzelAppIntents.appex", () => {
    const conf = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"));
    expect(conf.build.beforeBundleCommand).toContain("macos-siri/build-siri.sh");
    expect(conf.bundle.macOS.files["PlugIns/DemerzelAppIntents.appex"]).toContain(
      "DemerzelAppIntents.appex",
    );
  });

  test("production Swift intents exist", () => {
    expect(existsSync(join(root, "src-tauri/macos-siri/Sources/AskDemerzelStatusIntent.swift"))).toBe(
      true,
    );
    expect(existsSync(join(root, "src-tauri/macos-siri/Sources/RestartProcessIntent.swift"))).toBe(
      true,
    );
    expect(existsSync(join(root, "src-tauri/macos-siri/Sources/DemSockClient.swift"))).toBe(true);
    expect(existsSync(join(root, "src-tauri/macos-siri/build-siri.sh"))).toBe(true);
    expect(existsSync(join(root, "src-tauri/macos-siri/project.yml"))).toBe(true);
  });

  test("RestartProcess asks for confirmation; AskDemerzelStatus uses dem.sock pulse", () => {
    const restart = readFileSync(
      join(root, "src-tauri/macos-siri/Sources/RestartProcessIntent.swift"),
      "utf8",
    );
    const status = readFileSync(
      join(root, "src-tauri/macos-siri/Sources/AskDemerzelStatusIntent.swift"),
      "utf8",
    );
    expect(restart).toContain("requestConfirmation");
    expect(restart).toContain("restartDialog");
    expect(status).toContain("pulseDialog");
    expect(status).toContain("AskDemerzelStatusIntent");
  });
});
