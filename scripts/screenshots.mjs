// Снимает экраны приложения в docs/screenshots/ (#screenshots-on-test).
// Запускается из `bun test`: если Playwright недоступен — тихо пропускает шаг.
// Сборку фронтенда делает сам (`bun run build`) перед снимками; нужен Chromium для Playwright.
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "docs", "screenshots");
const PORT = 4173;

let chromium;
try {
  ({ chromium } = await import("playwright"));
} catch {
  console.log("screenshots: playwright не установлен — шаг пропущен (npm i -D playwright)");
  process.exit(0);
}

// Сборка всегда свежая: скриншоты должны показывать текущий код.
const build = spawnSync("bun", ["run", "build"], { cwd: root, stdio: "ignore" });
if (build.status !== 0) {
  console.log("screenshots: сборка не удалась — шаг пропущен");
  process.exit(0);
}

mkdirSync(out, { recursive: true });
const server = spawn("bun", ["run", "preview", "--port", String(PORT), "--strictPort"], {
  cwd: root,
  stdio: "ignore",
});

async function waitForServer(url, tries = 60) {
  for (let i = 0; i < tries; i++) {
    try {
      const r = await fetch(url);
      if (r.ok) return;
    } catch {
      /* ждём */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`preview не поднялся на ${url}`);
}

const shots = [
  { name: "columns-dark", theme: "dark", view: "columns" },
  { name: "columns-light", theme: "light", view: "columns" },
  { name: "tabs-dark", theme: "dark", view: "tabs" },
  { name: "tabs-light", theme: "light", view: "tabs" },
];

let browser;
try {
  await waitForServer(`http://localhost:${PORT}`);
  browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 }, deviceScaleFactor: 1 });
  await page.goto(`http://localhost:${PORT}`);
  for (const s of shots) {
    await page.evaluate((theme) => {
      localStorage.setItem("demerzel-theme", theme);
      document.documentElement.classList.remove("dark", "light");
      document.documentElement.classList.add(theme);
    }, s.theme);
    await page.evaluate((view) => localStorage.setItem("demerzel.view", view), s.view);
    await page.reload();
    await page.waitForTimeout(800);
    await page.screenshot({ path: join(out, `${s.name}.png`), fullPage: false });
    console.log(`screenshots: ${s.name}.png`);
  }
} catch (err) {
  console.log(`screenshots: пропущено (${err.message})`);
} finally {
  await browser?.close();
  server.kill();
}
