import { copyFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dest = join(root, "node_modules/tauri-plugin-widgets-api/swift/Package.swift");
const src = join(root, "scripts/TauriWidgets.Package.swift");
if (existsSync(src) && existsSync(dirname(dest))) {
  copyFileSync(src, dest);
  console.log("[spike] patched TauriWidgets Package.swift (no missing Tests path)");
}
