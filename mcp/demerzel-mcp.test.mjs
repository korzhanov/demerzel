import { test, expect } from "bun:test";
import { mkdtempSync, writeFileSync, mkdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { handle, discoverAutomations, listProjects, readConfig, isLikelyAutomation } from "./demerzel-mcp.mjs";

const tmp = (n) => mkdtempSync(join(tmpdir(), `dem-mcp-${n}-`));

test("initialize и tools/list", async () => {
  const init = await handle({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} });
  expect(init.result.serverInfo.name).toBe("demerzel");
  const list = await handle({ jsonrpc: "2.0", id: 2, method: "tools/list" });
  const names = list.result.tools.map((t) => t.name);
  expect(names).toContain("set_automation_selection");
  expect(names).toContain("start_project");
});

test("уведомления не получают ответа", async () => {
  expect(await handle({ jsonrpc: "2.0", method: "notifications/initialized" })).toBeNull();
});

test("неизвестный инструмент и метод — ошибка", async () => {
  expect((await handle({ jsonrpc: "2.0", id: 3, method: "tools/call", params: { name: "nope" } })).error.code).toBe(-32602);
  expect((await handle({ jsonrpc: "2.0", id: 4, method: "x" })).error.code).toBe(-32601);
});

test("discover из package.json с подсказкой", async () => {
  const d = tmp("disc");
  writeFileSync(join(d, "package.json"), JSON.stringify({ scripts: { build: "vite build", pipeline: "x" } }));
  const r = await discoverAutomations(d);
  expect(r.map((x) => x.name)).toEqual(["build", "pipeline"]);
  expect(r.find((x) => x.name === "pipeline").likely_automation).toBe(true);
  expect(isLikelyAutomation("build")).toBe(false);
});

test("set_automation_selection сохраняет прочие поля", async () => {
  const d = tmp("sel");
  writeFileSync(join(d, "orchestrator.json"), JSON.stringify({ name: "parser", commands: { dev: "x" } }));
  const out = await handle({
    jsonrpc: "2.0", id: 5, method: "tools/call",
    params: { name: "set_automation_selection", arguments: { path: d, selected: [{ source: "package.json", name: "pipeline" }] } },
  });
  expect(out.result.isError).toBe(false);
  const cfg = JSON.parse(readFileSync(join(d, "orchestrator.json"), "utf8"));
  expect(cfg.name).toBe("parser");
  expect(cfg.commands.dev).toBe("x");
  expect(cfg.automation.selected[0].name).toBe("pipeline");
  expect((await readConfig(d)).name).toBe("parser");
});

test("list_projects находит папки с маркерами", async () => {
  const root = tmp("roots");
  mkdirSync(join(root, "a"));
  writeFileSync(join(root, "a", "package.json"), "{}");
  mkdirSync(join(root, "empty"));
  const found = await listProjects([root]);
  expect(found.map((p) => p.name)).toEqual(["a"]);
});

test("dem не найден — понятная ошибка", async () => {
  process.env.DEM_BIN = "/nonexistent/dem-bin";
  const out = await handle({ jsonrpc: "2.0", id: 6, method: "tools/call", params: { name: "project_status", arguments: { path: "/tmp" } } });
  expect(out.result.isError).toBe(true);
  delete process.env.DEM_BIN;
});
