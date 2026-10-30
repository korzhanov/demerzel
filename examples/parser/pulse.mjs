#!/usr/bin/env bun
/** Пример stdout JSON пульса для 100words-style parser (#97). */
const parsed = 1500;
const checked = 450;
const payload = {
  summary: `${parsed} новых лемм, ${checked} проверено`,
  metrics: { parsed, checked },
  eta_seconds: 600,
  state: "running",
};
process.stdout.write(JSON.stringify(payload));
