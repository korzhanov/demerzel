/** Зеркало формата пульса в summary.json / виджете (#98) и контракта опроса (#97). */

export interface ProjectPulse {
  summary: string;
  eta_seconds?: number | null;
  state?: string | null;
  /** Числовые метрики (parsed / checked, …); виджет не рисует. */
  metrics?: Record<string, number> | null;
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

/** Валидация сырого JSON пульса (контракт #97). */
export function parsePulsePayload(raw: unknown): ProjectPulse {
  if (raw == null || typeof raw !== "object" || Array.isArray(raw)) {
    throw new Error("pulse must be a JSON object");
  }
  const obj = raw as Record<string, unknown>;
  const summary = typeof obj.summary === "string" ? obj.summary.trim() : "";
  if (!summary) {
    throw new Error("pulse.summary must not be empty");
  }
  const pulse: ProjectPulse = { summary };
  if (obj.eta_seconds != null) {
    if (typeof obj.eta_seconds !== "number" || !Number.isFinite(obj.eta_seconds)) {
      throw new Error("pulse.eta_seconds must be a number");
    }
    pulse.eta_seconds = Math.max(0, Math.floor(obj.eta_seconds));
  }
  if (obj.state != null) {
    if (typeof obj.state !== "string") {
      throw new Error("pulse.state must be a string");
    }
    const state = obj.state.trim();
    if (state) pulse.state = state;
  }
  if (obj.metrics != null) {
    if (typeof obj.metrics !== "object" || Array.isArray(obj.metrics)) {
      throw new Error("pulse.metrics must be an object");
    }
    const metrics: Record<string, number> = {};
    for (const [k, v] of Object.entries(obj.metrics as Record<string, unknown>)) {
      if (typeof v !== "number" || !Number.isFinite(v)) {
        throw new Error(`pulse.metrics.${k} must be a number`);
      }
      metrics[k] = Math.trunc(v);
    }
    if (Object.keys(metrics).length > 0) pulse.metrics = metrics;
  }
  return pulse;
}
