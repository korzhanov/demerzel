import { automationIsRunning, automationStatusText } from "./automation";
import { describe, expect, test } from "bun:test";
import { initialSelection, isChecked, toggle } from "./automation";
import type { AutomationsView } from "./types";

const view: AutomationsView = {
  discovered: [
    { source: "package.json", name: "pipeline", command: "bun x", likely_automation: true },
    { source: "package.json", name: "build", command: "vite build", likely_automation: false },
  ],
  selected: [],
  schedules: [],
  restart: { enabled: false, max_retries: 3 },
};

describe("automation selection", () => {
  test("без сохранённого выбора предвыбирает эвристику", () => {
    expect(initialSelection(view)).toEqual([{ source: "package.json", name: "pipeline" }]);
  });
  test("сохранённый выбор важнее эвристики и игнорирует пропавшие пункты", () => {
    const sel = initialSelection({
      ...view,
      selected: [{ source: "package.json", name: "build" }, { source: "gone", name: "x" }],
    });
    expect(sel).toEqual([{ source: "package.json", name: "build" }]);
  });
  test("toggle добавляет и убирает", () => {
    const build = view.discovered[1];
    const on = toggle([], build);
    expect(isChecked(on, build)).toBe(true);
    expect(toggle(on, build)).toEqual([]);
  });
});

describe("automation status", () => {
  const base = { source: "s", name: "n", command: "cmd", run_id: 1 };
  test("текст статуса", () => {
    expect(automationStatusText({ ...base, command: null, run_id: null, status: null })).toBe("не найдена в проекте");
    expect(automationStatusText({ ...base, run_id: null, status: null })).toBe("не запускалась");
    expect(automationStatusText({ ...base, status: { Exited: 2 } })).toBe("Exited(2)");
  });
  test("запущена ли", () => {
    expect(automationIsRunning({ ...base, status: "Running" })).toBe(true);
    expect(automationIsRunning({ ...base, status: { Exited: 0 } })).toBe(false);
  });
});

import { shouldRestart } from "./automation";
describe("restart rule", () => {
  const on = { enabled: true, max_retries: 2 };
  test("выключено по умолчанию и без попыток сверх лимита", () => {
    expect(shouldRestart({ Exited: 1 }, 0, { enabled: false, max_retries: 2 })).toBe(false);
    expect(shouldRestart({ Exited: 1 }, 2, on)).toBe(false);
  });
  test("перезапуск только при сбое", () => {
    expect(shouldRestart({ Exited: 1 }, 0, on)).toBe(true);
    expect(shouldRestart("Crashed", 1, on)).toBe(true);
    expect(shouldRestart({ Exited: 0 }, 0, on)).toBe(false);
    expect(shouldRestart("Running", 0, on)).toBe(false);
  });
});
