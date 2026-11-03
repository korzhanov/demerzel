<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { AgentCatalogEntry, AgentRunner } from "$lib/types";
  import { AGENT_RUNNERS } from "$lib/types";
  import { agentRunnerLabel } from "$lib/agents";
  import {
    buildInstallCommand,
    installRowFor,
    mergeCatalogPrefs,
    reorderCatalog,
    setCatalogEnabled,
    type AgentCatalogPref,
  } from "$lib/agent-install";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import XIcon from "@lucide/svelte/icons/x";
  import GripVerticalIcon from "@lucide/svelte/icons/grip-vertical";
  import BotIcon from "@lucide/svelte/icons/bot";

  let {
    open = $bindable(false),
    catalog = $bindable<AgentCatalogEntry[]>([]),
    prefs = $bindable<AgentCatalogPref[]>([]),
    onPersist,
  }: {
    open?: boolean;
    catalog?: AgentCatalogEntry[];
    prefs?: AgentCatalogPref[];
    onPersist: (prefs: AgentCatalogPref[]) => void | Promise<void>;
  } = $props();

  let error = $state("");
  let confirmRunner = $state<AgentRunner | null>(null);
  let busy = $state(false);
  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);

  const merged = $derived(mergeCatalogPrefs(prefs, AGENT_RUNNERS));
  const availableMap = $derived(
    new Map(catalog.map((e) => [e.runner, e] as const)),
  );

  const vendorByRunner: Record<AgentRunner, string> = {
    claude: "Anthropic",
    acp: "Свой агент (ACP)",
    cursor_agent: "Cursor",
    codex: "OpenAI",
    hermes: "Nous Research",
    opencode: "Anomaly",
    gemini: "Google",
    aider: "Aider",
    ollama: "Ollama",
    grok: "xAI",
  };

  function subtitleFor(runner: AgentRunner, available: boolean): string {
    const vendor = vendorByRunner[runner] ?? runner;
    const row = installRowFor(runner);
    if (!available) {
      if (row && !row.verified) return `${vendor} · команда не сверена`;
      return `${vendor} · не установлен`;
    }
    if (row && !row.headless_supported) return `${vendor} · только детект`;
    return vendor;
  }

  async function persist(next: AgentCatalogPref[]) {
    prefs = next;
    await onPersist(next);
  }

  async function refreshCatalog() {
    catalog = await invoke<AgentCatalogEntry[]>("list_available_agents");
  }

  async function confirmInstall() {
    const runner = confirmRunner;
    if (!runner) return;
    busy = true;
    error = "";
    try {
      await invoke<string>("open_agent_install_terminal", { runner });
      confirmRunner = null;
      await refreshCatalog();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /** Индекс строки под курсором — без HTML5 DnD (WKWebView/Tauri его глушит). */
  function rowIndexFromPoint(x: number, y: number): number | null {
    const el = document.elementFromPoint(x, y);
    const row = el?.closest<HTMLElement>("[data-agent-row-index]");
    if (!row) return null;
    const n = Number(row.dataset.agentRowIndex);
    return Number.isFinite(n) ? n : null;
  }

  function onGripPointerDown(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    const grip = e.currentTarget as HTMLElement;
    grip.setPointerCapture(e.pointerId);
    dragFrom = i;
    dragOver = i;

    function onMove(ev: PointerEvent) {
      const over = rowIndexFromPoint(ev.clientX, ev.clientY);
      if (over != null) dragOver = over;
    }

    async function onUp(ev: PointerEvent) {
      grip.releasePointerCapture(ev.pointerId);
      grip.removeEventListener("pointermove", onMove);
      grip.removeEventListener("pointerup", onUp);
      grip.removeEventListener("pointercancel", onUp);

      const from = dragFrom;
      const to = rowIndexFromPoint(ev.clientX, ev.clientY) ?? dragOver;
      dragFrom = null;
      dragOver = null;
      if (from == null || to == null || from === to) return;
      await persist(reorderCatalog(merged, from, to));
    }

    grip.addEventListener("pointermove", onMove);
    grip.addEventListener("pointerup", onUp);
    grip.addEventListener("pointercancel", onUp);
  }

  function close() {
    open = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (confirmRunner) confirmRunner = null;
      else close();
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (open) onKeydown(e);
  }}
/>

{#if open}
  <!-- Не <dialog showModal>: top-layer перекрывает тултипы и Cursor browser overlays. -->
  <div
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/45 p-3"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) close();
    }}
  >
    <div
      class="flex max-h-[min(90vh,42rem)] w-[min(32rem,calc(100vw-1.5rem))] flex-col overflow-hidden rounded-lg border border-border bg-card text-card-foreground shadow-lg"
      role="dialog"
      aria-modal="true"
      aria-labelledby="agents-settings-title"
    >
      <header class="flex items-start gap-2 border-b border-border px-4 py-3">
        <div class="min-w-0 flex-1 space-y-1">
          <h2
            id="agents-settings-title"
            class="flex items-center gap-2 text-base font-medium tracking-tight"
          >
            <BotIcon class="size-4 text-primary" aria-hidden="true" />
            Агенты
          </h2>
          <p class="text-sm text-muted-foreground">
            Приоритет — номер и порядок. Тяните за ⋮⋮. Установка → Terminal.app.
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          class="size-8 p-0"
          aria-label="Закрыть"
          onclick={close}
        >
          <XIcon class="size-4" />
        </Button>
      </header>

      <div class="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-3">
        {#if error}
          <p class="text-sm text-destructive" role="alert">{error}</p>
        {/if}

        <section class="space-y-2" aria-labelledby="agents-list-title">
          <h3
            id="agents-list-title"
            class="px-1 text-[13px] font-medium text-muted-foreground"
          >
            Список
          </h3>
          <ul class="overflow-hidden rounded-xl border border-border bg-muted/20">
            {#each merged as pref, i (pref.runner)}
              {@const entry = availableMap.get(pref.runner)}
              {@const available = Boolean(entry?.available)}
              {@const canInstall =
                Boolean(installRowFor(pref.runner)?.install) && !available}
              <li
                data-agent-row-index={i}
                class={[
                  "flex items-center gap-2 border-b border-border px-2.5 py-2.5 last:border-b-0",
                  !pref.enabled ? "opacity-70" : "",
                  dragOver === i && dragFrom != null && dragFrom !== i
                    ? "bg-primary/10"
                    : "",
                  dragFrom === i ? "opacity-50" : "",
                ].join(" ")}
              >
                <span
                  class="w-5 shrink-0 text-center font-mono text-[11px] tabular-nums text-muted-foreground"
                  aria-hidden="true"
                >
                  {i + 1}
                </span>
                <button
                  type="button"
                  class="inline-flex shrink-0 cursor-grab touch-none text-muted-foreground hover:text-foreground active:cursor-grabbing"
                  aria-label={`Перетащить ${agentRunnerLabel(pref.runner)}`}
                  onpointerdown={(e) => onGripPointerDown(e, i)}
                >
                  <GripVerticalIcon class="size-4" />
                </button>
                <div class="min-w-0 flex-1">
                  <p class="truncate text-sm font-medium leading-tight">
                    {agentRunnerLabel(pref.runner)}
                  </p>
                  <p class="truncate text-[11px] text-muted-foreground">
                    {subtitleFor(pref.runner, available)}
                  </p>
                </div>

                {#if canInstall}
                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    class="h-7 shrink-0 rounded-full px-3 text-xs font-medium"
                    onclick={() => (confirmRunner = pref.runner)}
                  >
                    Установить
                  </Button>
                {/if}

                <Switch
                  checked={pref.enabled}
                  onCheckedChange={(v) =>
                    void persist(
                      setCatalogEnabled(merged, pref.runner, v === true),
                    )}
                  aria-label={`Использовать ${agentRunnerLabel(pref.runner)}`}
                />
              </li>
            {/each}
          </ul>
        </section>
      </div>
    </div>
  </div>
{/if}

{#if confirmRunner}
  {@const cmd = buildInstallCommand(confirmRunner)}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) confirmRunner = null;
    }}
  >
    <div
      class="w-[min(28rem,calc(100vw-2rem))] space-y-3 rounded-lg border border-border bg-card p-4 shadow-xl"
      role="dialog"
      aria-modal="true"
      aria-label="Подтверждение установки"
    >
      <h3 class="text-sm font-medium">
        Установить {agentRunnerLabel(confirmRunner)}?
      </h3>
      <p class="text-xs text-muted-foreground">
        Откроется Terminal.app с командой. Запуск — только после вашего Enter в
        терминале.
      </p>
      <pre
        class="overflow-x-auto rounded-md border border-border bg-muted/40 p-2 font-mono text-[11px]"
        >{cmd}</pre
      >
      <div class="flex justify-end gap-2">
        <Button
          type="button"
          size="sm"
          variant="outline"
          onclick={() => (confirmRunner = null)}>Отмена</Button
        >
        <Button
          type="button"
          size="sm"
          disabled={busy || !cmd}
          onclick={() => void confirmInstall()}>Открыть Terminal</Button
        >
      </div>
    </div>
  </div>
{/if}
