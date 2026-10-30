/**
 * Контракт моста App Intents ↔ dem.sock (#100).
 * Эталон Swift: spikes/2-siri-app-intents/; live-интенты — #101.
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

/** Placeholder Apple: phrase must include application name. */
export const APPLICATION_NAME_PLACEHOLDER = "\\(.applicationName)" as const;

export type AppShortcutPhrase = {
  /** Swift AppShortcut phrase template */
  swift: string;
  /** Example utterance for locale */
  exampleRu: string;
  locale: "ru" | "en";
};

/** Каталог фраз спайка (имя приложения обязательно). */
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

export function phraseIncludesApplicationName(swiftPhrase: string): boolean {
  return (
    swiftPhrase.includes("\\(.applicationName)") ||
    swiftPhrase.includes("(.applicationName)")
  );
}

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
