import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import process from "node:process";
import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";
const host = process.env.TAURI_DEV_HOST;

// Номер сборки для UI: версия из VERSION и короткий коммит (#build-label).
const appVersion = readFileSync("VERSION", "utf8").trim();
let appCommit = "";
try {
  const run = (cmd) => execSync(cmd, { stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
  appCommit = run("git rev-parse --short HEAD");
  // Незакоммиченные правки помечаются, чтобы сборку не путали с коммитом.
  if (run("git status --porcelain")) appCommit += "+dirty";
} catch {
  appCommit = "";
}

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [tailwindcss(), sveltekit()],
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
    __APP_COMMIT__: JSON.stringify(appCommit),
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
