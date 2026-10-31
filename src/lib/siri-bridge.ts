/**
 * Контракт моста App Intents ↔ dem.sock (#101).
 * Swift-реализация: src-tauri/macos-siri/ (спайк #100 удалён, история в git).
 */

export type DemSockAction = "pulse" | "restart" | "status" | "up" | "down" | "sync" | "watch";

export type DemSockRequest = {
  action: DemSockAction;
  root: string;
  project?: string;
  json?: boolean;
};

export type DemSockEnvelope = {
  output: string;
  error: string | null;
};

export type PulseJsonForDialog = {
  project?: string;
  summary: string;
  eta_seconds?: number;
  state?: string;
};

export type RestartJsonForDialog = {
  project: string;
  status: string;
  id?: number;
};

/** Placeholder Apple: phrase must include application name. */
export const APPLICATION_NAME_PLACEHOLDER = "\\(.applicationName)" as const;

export type AppShortcutPhrase = {
  /** Swift AppShortcut phrase template */
  swift: string;
  /** Example utterance for locale */
  exampleRu: string;
  locale: "ru" | "en";
};

/** Каталог фраз AskDemerzelStatus (имя приложения обязательно). */
export const SIRI_ASK_PHRASES: AppShortcutPhrase[] = [
  {
    swift: "Спроси \\(.applicationName) про \\(\\.$project)",
    exampleRu: "Спроси Demerzel про parser",
    locale: "ru",
  },
  {
    swift: "Спроси \\(.applicationName) о \\(\\.$project)",
    exampleRu: "Спроси Demerzel о parser",
    locale: "ru",
  },
  {
    swift: "Статус \\(.applicationName) \\(\\.$project)",
    exampleRu: "Статус Demerzel parser",
    locale: "ru",
  },
  {
    swift: "Ask \\(.applicationName) about \\(\\.$project)",
    exampleRu: "Ask Demerzel about parser",
    locale: "en",
  },
];

/** Каталог фраз RestartProcess (имя приложения обязательно). */
export const SIRI_RESTART_PHRASES: AppShortcutPhrase[] = [
  {
    swift: "Перезапусти \\(.applicationName) \\(\\.$project)",
    exampleRu: "Перезапусти Demerzel parser",
    locale: "ru",
  },
  {
    swift: "Restart \\(.applicationName) \\(\\.$project)",
    exampleRu: "Restart Demerzel parser",
    locale: "en",
  },
];

/** Production intents (#101). */
export const SIRI_PRODUCTION_INTENTS = [
  "AskDemerzelStatus",
  "RestartProcess",
] as const;

export type SiriProductionIntent = (typeof SIRI_PRODUCTION_INTENTS)[number];

export function formatEtaRu(secs: number): string {
  if (secs < 60) return `ещё ${secs} с`;
  if (secs < 3600) return `ещё ${Math.floor((secs + 59) / 60)} мин`;
  return `ещё ${Math.floor((secs + 3599) / 3600)} ч`;
}

/** Текст IntentDialog из pulse JSON (как DemSockClient.pulseDialog). */
export function formatPulseDialog(
  pulse: PulseJsonForDialog,
  fallbackProject?: string,
): string {
  const name = pulse.project?.trim() || fallbackProject?.trim() || "проект";
  const summary = pulse.summary.trim();
  let dialog = `${name}: ${summary}`;
  if (typeof pulse.eta_seconds === "number") {
    dialog += `, ${formatEtaRu(pulse.eta_seconds)}`;
  }
  return dialog;
}

/** Текст IntentDialog после restart JSON. */
export function formatRestartDialog(restart: RestartJsonForDialog): string {
  const name = restart.project.trim() || "проект";
  const status = restart.status.trim() || "restarted";
  return `${name}: ${status}`;
}

export function buildPulseSockRequest(opts: {
  root: string;
  project?: string;
}): DemSockRequest {
  return {
    action: "pulse",
    root: opts.root,
    project: opts.project,
    json: true,
  };
}

export function buildRestartSockRequest(opts: {
  root: string;
  project: string;
}): DemSockRequest {
  return {
    action: "restart",
    root: opts.root,
    project: opts.project,
    json: true,
  };
}

/** Parse supervisor envelope; throw if error field set. */
export function parseSockEnvelope(raw: string): DemSockEnvelope {
  const env = JSON.parse(raw) as DemSockEnvelope;
  if (typeof env.output !== "string") {
    throw new Error("dem.sock: invalid envelope (output)");
  }
  return {
    output: env.output,
    error: env.error ?? null,
  };
}

export function pulseFromSockOutput(output: string): PulseJsonForDialog {
  const obj = JSON.parse(output) as PulseJsonForDialog;
  if (typeof obj.summary !== "string") {
    throw new Error("dem.sock: pulse JSON missing summary");
  }
  return obj;
}

export function restartFromSockOutput(output: string): RestartJsonForDialog {
  const obj = JSON.parse(output) as RestartJsonForDialog;
  if (typeof obj.project !== "string" || typeof obj.status !== "string") {
    throw new Error("dem.sock: restart JSON missing project/status");
  }
  return obj;
}

export function phraseIncludesApplicationName(swiftPhrase: string): boolean {
  return (
    swiftPhrase.includes("\\(.applicationName)") ||
    swiftPhrase.includes("(.applicationName)")
  );
}

/** RestartProcess требует подтверждение (ConfirmationBehavior.required). */
export function restartRequiresConfirmation(): boolean {
  return true;
}

export function productionIntents(): {
  status: SiriProductionIntent;
  restart: SiriProductionIntent;
  transport: "unix-socket";
  appex: string;
  confirmation: boolean;
} {
  return {
    status: "AskDemerzelStatus",
    restart: "RestartProcess",
    transport: "unix-socket",
    appex: "DemerzelAppIntents.appex",
    confirmation: restartRequiresConfirmation(),
  };
}

/** @deprecated spike #100 — next was #101; use productionIntents(). */
export function spikeVerdict(): {
  status: "go" | "no-go";
  transport: "unix-socket";
  nextIssue: number;
} {
  return {
    status: "go",
    transport: "unix-socket",
    nextIssue: 101,
  };
}
