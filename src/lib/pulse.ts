/** Зеркало формата пульса в summary.json / виджете (#98). Источник опроса — #97. */

export interface ProjectPulse {
  summary: string;
  eta_seconds?: number | null;
  state?: string | null;
}

export interface ProjectWithPulse {
  name: string;
  pulse?: ProjectPulse | null;
}

/** Человекочитаемый ETA (RU), зеркало `format_eta_ru`. */
export function formatEtaRu(secs: number): string {
  if (secs < 60) return `ещё ${secs} с`;
  if (secs < 3600) {
    const mins = Math.ceil(secs / 60);
    return `ещё ${mins} мин`;
  }
  const hours = Math.ceil(secs / 3600);
  return `ещё ${hours} ч`;
}

/** `name: summary` или `name: summary, ещё N мин`. */
export function formatPulseLine(name: string, pulse: ProjectPulse): string {
  const summary = pulse.summary.trim();
  if (pulse.eta_seconds != null && Number.isFinite(pulse.eta_seconds)) {
    return `${name}: ${summary}, ${formatEtaRu(pulse.eta_seconds)}`;
  }
  return `${name}: ${summary}`;
}

/** Проекты с непустым текстом пульса. */
export function projectsWithPulse(
  projects: ProjectWithPulse[],
): Array<{ name: string; pulse: ProjectPulse }> {
  const out: Array<{ name: string; pulse: ProjectPulse }> = [];
  for (const p of projects) {
    const pulse = p.pulse;
    if (!pulse) continue;
    if (!pulse.summary.trim()) continue;
    out.push({ name: p.name, pulse });
  }
  return out;
}

/** Отпечаток текста пульса — для решения «обновлять ли виджет». */
export function pulseFingerprint(projects: ProjectWithPulse[]): string {
  return projectsWithPulse(projects)
    .map(({ name, pulse }) => formatPulseLine(name, pulse))
    .join("\n");
}
