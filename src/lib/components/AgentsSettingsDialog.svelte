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
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import XIcon from "@lucide/svelte/icons/x";
  import ArrowUpIcon from "@lucide/svelte/icons/arrow-up";
  import ArrowDownIcon from "@lucide/svelte/icons/arrow-down";
  import DownloadIcon from "@lucide/svelte/icons/download";
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

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let error = $state("");
  let confirmRunner = $state<AgentRunner | null>(null);
  let busy = $state(false);
  let dragFrom = $state<number | null>(null);

  const merged = $derived(mergeCatalogPrefs(prefs, AGENT_RUNNERS));
  const availableMap = $derived(
    new Map(catalog.map((e) => [e.runner, e] as const)),
  );
  const used = $derived(merged);
  const toInstall = $derived(
    merged.filter((p) => {
      const e = availableMap.get(p.runner);
      if (e?.available) return false;
      const row = installRowFor(p.runner);
      return Boolean(row?.install);
    }),
  );

  $effect(() => {
    const el = dialogEl;
    if (!el) return;
    if (open) {
      if (!el.open) el.showModal();
    } else if (el.open) {
      el.close();
    }
  });

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

  function onDragStart(i: number) {
    dragFrom = i;
  }

  async function onDrop(to: number) {
    if (dragFrom == null) return;
    const next = reorderCatalog(merged, dragFrom, to);
    dragFrom = null;
    await persist(next);
  }
</script>

<dialog
  bind:this={dialogEl}
  class="fixed inset-0 z-50 m-auto h-fit max-h-[min(90vh,42rem)] w-[min(32rem,calc(100vw-1.5rem))] overflow-hidden rounded-lg border border-border bg-card p-0 text-card-foreground shadow-lg backdrop:bg-black/55"
  aria-labelledby="agents-settings-title"
  onclose={() => (open = false)}
  onclick={(e) => {
    if (e.target === dialogEl) open = false;
  }}
>
  <Tooltip.Provider delayDuration={250}>
    <div class="flex max-h-[min(90vh,42rem)] flex-col">
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
            Порядок = приоритет. Установка открывает Terminal.app после подтверждения.
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          class="size-8 p-0"
          aria-label="Закрыть"
          onclick={() => (open = false)}
        >
          <XIcon class="size-4" />
        </Button>
      </header>

      <div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-3">
        {#if error}
          <p class="text-sm text-destructive" role="alert">{error}</p>
        {/if}

        <section class="space-y-2" aria-labelledby="agents-used-title">
          <h3 id="agents-used-title" class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Используемые
          </h3>
          <ul class="space-y-1 rounded-md border border-border p-1">
            {#each used as pref, i (pref.runner)}
              {@const entry = availableMap.get(pref.runner)}
              <li
                class="flex items-center gap-1 rounded-md px-1 py-1 hover:bg-accent/40"
                draggable="true"
                ondragstart={() => onDragStart(i)}
                ondragover={(e) => e.preventDefault()}
                ondrop={() => void onDrop(i)}
              >
                <Checkbox
                  checked={pref.enabled}
                  onCheckedChange={(v) =>
                    void persist(setCatalogEnabled(merged, pref.runner, v === true))}
                  aria-label={`Использовать ${agentRunnerLabel(pref.runner)}`}
                />
                <span class="min-w-0 flex-1 truncate text-sm">
                  {agentRunnerLabel(pref.runner)}
                  {#if entry?.available}
                    <span class="text-muted-foreground">· ok</span>
                  {:else}
                    <span class="text-muted-foreground">· нет</span>
                  {/if}
                  {#if installRowFor(pref.runner)?.verified === false}
                    <span class="text-primary">· ?</span>
                  {/if}
                </span>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  class="size-7 p-0"
                  aria-label="Выше"
                  disabled={i === 0}
                  onclick={() => void persist(reorderCatalog(merged, i, i - 1))}
                >
                  <ArrowUpIcon class="size-3.5" />
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  class="size-7 p-0"
                  aria-label="Ниже"
                  disabled={i >= used.length - 1}
                  onclick={() => void persist(reorderCatalog(merged, i, i + 1))}
                >
                  <ArrowDownIcon class="size-3.5" />
                </Button>
              </li>
            {/each}
          </ul>
        </section>

        <section class="space-y-2" aria-labelledby="agents-install-title">
          <h3
            id="agents-install-title"
            class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
          >
            Доступные для установки
          </h3>
          {#if toInstall.length === 0}
            <p class="text-sm text-muted-foreground">Все поддерживаемые агенты уже на PATH.</p>
          {:else}
            <ul class="space-y-1 rounded-md border border-border p-1">
              {#each toInstall as pref (pref.runner)}
                {@const cmd = buildInstallCommand(pref.runner)}
                {@const row = installRowFor(pref.runner)}
                <li class="flex items-center gap-2 rounded-md px-2 py-1.5">
                  <div class="min-w-0 flex-1">
                    <p class="text-sm font-medium">{agentRunnerLabel(pref.runner)}</p>
                    <p class="truncate font-mono text-[11px] text-muted-foreground" title={cmd ?? ""}>
                      {cmd}
                    </p>
                    {#if row && !row.verified}
                      <p class="text-[11px] text-primary">Команда не сверена с docs.x.ai</p>
                    {/if}
                  </div>
                  <Tooltip.Root>
                    <Tooltip.Trigger>
                      {#snippet child({ props })}
                        <Button
                          {...props}
                          type="button"
                          size="sm"
                          variant="outline"
                          class="size-8 p-0"
                          aria-label="Установить"
                          onclick={() => (confirmRunner = pref.runner)}
                        >
                          <DownloadIcon class="size-4" />
                        </Button>
                      {/snippet}
                    </Tooltip.Trigger>
                    <Tooltip.Content>Установить через Terminal.app</Tooltip.Content>
                  </Tooltip.Root>
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      </div>
    </div>
  </Tooltip.Provider>
</dialog>

{#if confirmRunner}
  {@const cmd = buildInstallCommand(confirmRunner)}
  <dialog
    open
    class="fixed inset-0 z-[60] m-auto h-fit w-[min(28rem,calc(100vw-2rem))] rounded-lg border border-border bg-card p-4 shadow-xl backdrop:bg-black/40"
    aria-label="Подтверждение установки"
    onclick={(e) => {
      if (e.currentTarget === e.target) confirmRunner = null;
    }}
  >
    <div class="space-y-3">
      <h3 class="text-sm font-medium">Установить {agentRunnerLabel(confirmRunner)}?</h3>
      <p class="text-xs text-muted-foreground">
        Откроется Terminal.app с командой. Запуск — только после вашего Enter в терминале.
      </p>
      <pre class="overflow-x-auto rounded-md border border-border bg-muted/40 p-2 font-mono text-[11px]">{cmd}</pre>
      <div class="flex justify-end gap-2">
        <Button type="button" size="sm" variant="outline" onclick={() => (confirmRunner = null)}
          >Отмена</Button
        >
        <Button type="button" size="sm" disabled={busy || !cmd} onclick={() => void confirmInstall()}
          >Открыть Terminal</Button
        >
      </div>
    </div>
  </dialog>
{/if}
