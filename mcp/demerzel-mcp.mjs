#!/usr/bin/env node
// MCP-сервер Demerzel (stdio, JSON-RPC 2.0, без зависимостей) — #158.
// Инструменты читают orchestrator.json, ищут автозадачи в package.json и вызывают CLI `dem`.
// Переменные окружения: DEM_BIN (путь к `dem`, по умолчанию `dem`), DEMERZEL_ROOTS (папки через `:`).
import { execFile } from "node:child_process";
import { readFile, writeFile, readdir, stat } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, resolve, basename } from "node:path";
import { createInterface } from "node:readline";

export const PROTOCOL = "2024-11-05";
const PROJECT_MARKERS = ["package.json", "orchestrator.json", "Cargo.toml", "go.mod", "pyproject.toml", "requirements.txt", "Makefile", "auto-run.json", ".git"];
const HINTS = ["pipeline", "parse", "sync", "watch", "cron", "job", "worker", "auto", "schedule", "daemon"];

export const isLikelyAutomation = (name) => HINTS.some((h) => name.toLowerCase().includes(h));

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

export async function readConfig(dir) {
  const file = join(dir, "orchestrator.json");
  if (!existsSync(file)) return null;
  return readJson(file);
}

export async function discoverAutomations(dir) {
  const out = [];
  const pkg = join(dir, "package.json");
  if (existsSync(pkg)) {
    const scripts = (await readJson(pkg)).scripts ?? {};
    for (const name of Object.keys(scripts).sort()) {
      out.push({ source: "package.json", name, command: `bun run ${name}`, likely_automation: isLikelyAutomation(name) });
    }
  }
  return out;
}

export async function listProjects(roots) {
  const found = [];
  for (const root of roots) {
    if (!existsSync(root)) continue;
    for (const entry of await readdir(root)) {
      const p = join(root, entry);
      if (!(await stat(p)).isDirectory()) continue;
      if (PROJECT_MARKERS.some((m) => existsSync(join(p, m)))) found.push({ name: basename(p), path: p });
    }
  }
  return found;
}

function runDem(args) {
  const bin = process.env.DEM_BIN || "dem";
  return new Promise((res) => {
    execFile(bin, args, { timeout: 60_000 }, (err, stdout, stderr) => {
      const text = (stdout + stderr).trim();
      if (err && err.code === "ENOENT") return res({ ok: false, text: `CLI \`${bin}\` не найден. Установите dem или задайте DEM_BIN.` });
      res({ ok: !err, text: text || (err ? String(err.message) : "ok") });
    });
  });
}

const str = (desc) => ({ type: "string", description: desc });
const pathProp = str("Абсолютный путь к папке проекта");

export const TOOLS = [
  {
    name: "list_projects",
    description: "Найти проекты в папках (DEMERZEL_ROOTS или roots). Ищет маркеры: package.json, orchestrator.json, Cargo.toml и др.",
    inputSchema: { type: "object", properties: { roots: { type: "array", items: str("папка") } } },
    run: async ({ roots }) => listProjects(roots?.length ? roots : (process.env.DEMERZEL_ROOTS || "").split(":").filter(Boolean)),
  },
  {
    name: "project_config",
    description: "Прочитать orchestrator.json проекта (или null, если файла нет).",
    inputSchema: { type: "object", properties: { path: pathProp }, required: ["path"] },
    run: async ({ path }) => readConfig(resolve(path)),
  },
  {
    name: "discover_automations",
    description: "Скрипты package.json как кандидаты автозадач. Только чтение. likely_automation — подсказка по имени.",
    inputSchema: { type: "object", properties: { path: pathProp }, required: ["path"] },
    run: async ({ path }) => discoverAutomations(resolve(path)),
  },
  {
    name: "set_automation_selection",
    description: "Записать выбранные автозадачи в orchestrator.json → automation.selected. Остальные поля не трогаются.",
    inputSchema: {
      type: "object",
      properties: {
        path: pathProp,
        selected: { type: "array", items: { type: "object", properties: { source: str("источник"), name: str("имя") }, required: ["source", "name"] } },
      },
      required: ["path", "selected"],
    },
    run: async ({ path, selected }) => {
      const dir = resolve(path);
      const file = join(dir, "orchestrator.json");
      const doc = existsSync(file) ? await readJson(file) : { name: basename(dir) };
      doc.automation = { selected };
      await writeFile(file, JSON.stringify(doc, null, 2) + "\n");
      return { ok: true, file };
    },
  },
  {
    name: "project_status",
    description: "Статус проекта через `dem status`.",
    inputSchema: { type: "object", properties: { path: pathProp }, required: ["path"] },
    run: async ({ path }) => runDem(["status", resolve(path)]),
  },
  {
    name: "start_project",
    description: "Поднять сервисы проекта через `dem up`.",
    inputSchema: { type: "object", properties: { path: pathProp }, required: ["path"] },
    run: async ({ path }) => runDem(["up", resolve(path)]),
  },
  {
    name: "stop_project",
    description: "Остановить сервисы проекта через `dem down`.",
    inputSchema: { type: "object", properties: { path: pathProp }, required: ["path"] },
    run: async ({ path }) => runDem(["down", resolve(path)]),
  },
  {
    name: "pulse",
    description: "Прогресс проекта (JSON) через `dem pulse --json`.",
    inputSchema: { type: "object", properties: { path: pathProp, project: str("имя сервиса (необязательно)") }, required: ["path"] },
    run: async ({ path, project }) => runDem(["pulse", ...(project ? [project] : []), "--json", resolve(path)]),
  },
  {
    name: "restart_service",
    description: "Перезапустить один сервис через `dem restart <service> --json`.",
    inputSchema: { type: "object", properties: { path: pathProp, service: str("имя сервиса") }, required: ["path", "service"] },
    run: async ({ path, service }) => runDem(["restart", service, "--json", resolve(path)]),
  },
];

const byName = new Map(TOOLS.map((t) => [t.name, t]));

export async function handle(msg) {
  const { id, method, params } = msg;
  const reply = (result) => ({ jsonrpc: "2.0", id, result });
  const fail = (code, message) => ({ jsonrpc: "2.0", id, error: { code, message } });
  if (id === undefined) return null; // уведомления
  if (method === "initialize") {
    return reply({ protocolVersion: PROTOCOL, capabilities: { tools: {} }, serverInfo: { name: "demerzel", version: "0.1.0" } });
  }
  if (method === "ping") return reply({});
  if (method === "tools/list") {
    return reply({ tools: TOOLS.map(({ name, description, inputSchema }) => ({ name, description, inputSchema })) });
  }
  if (method === "tools/call") {
    const tool = byName.get(params?.name);
    if (!tool) return fail(-32602, `неизвестный инструмент: ${params?.name}`);
    try {
      const out = await tool.run(params.arguments ?? {});
      const text = typeof out === "string" ? out : JSON.stringify(out, null, 2);
      return reply({ content: [{ type: "text", text }], isError: out && out.ok === false });
    } catch (e) {
      return reply({ content: [{ type: "text", text: String(e?.message ?? e) }], isError: true });
    }
  }
  return fail(-32601, `метод не поддерживается: ${method}`);
}

// Точка входа: stdio
if (process.argv[1] && import.meta.url === new URL(`file://${resolve(process.argv[1])}`).href) {
  const rl = createInterface({ input: process.stdin });
  rl.on("line", async (line) => {
    if (!line.trim()) return;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "parse error" } }) + "\n");
      return;
    }
    const out = await handle(msg);
    if (out) process.stdout.write(JSON.stringify(out) + "\n");
  });
}
