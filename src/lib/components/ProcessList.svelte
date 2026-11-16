<script lang="ts">
  import type { PortRow, ProcessRow } from "$lib/types";
  import { attachablePorts, localhostUrl, processActions, processOriginLabel, processStateLabel } from "$lib/process-actions";
  import { shortProjectLabel } from "$lib/ports";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import PlayIcon from "@lucide/svelte/icons/play";
  import PauseIcon from "@lucide/svelte/icons/pause";
  import SquareIcon from "@lucide/svelte/icons/square";
  import RotateCwIcon from "@lucide/svelte/icons/rotate-cw";
  import ScrollTextIcon from "@lucide/svelte/icons/scroll-text";
  import PlugIcon from "@lucide/svelte/icons/plug";
  import ExternalLinkIcon from "@lucide/svelte/icons/external-link";

  let {
    rows,
    ports,
    selectedId = null,
    busyId = null,
    onSelect,
    onStart,
    onPause,
    onResume,
    onStop,
    onRestart,
    onLogs,
    onOpen,
    onAttach,
  }: {
    rows: ProcessRow[];
    ports: PortRow[];
    selectedId?: number | null;
    busyId?: number | null;
    onSelect: (id: number) => void;
    onStart: (row: ProcessRow) => void;
    onPause: (row: ProcessRow) => void;
    onResume: (row: ProcessRow) => void;
    onStop: (row: ProcessRow) => void;
    onRestart: (row: ProcessRow) => void;
    onLogs: (row: ProcessRow) => void;
    onOpen: (port: number) => void;
    onAttach: (port: PortRow) => void;
  } = $props();

  const attachable = $derived(attachablePorts(ports));
</script>

<Tooltip.Provider delayDuration={300}>
  {#if rows.length === 0}
    <p class="text-sm text-muted-foreground">
      Нет процессов Demerzel. Запустите проект кнопкой ▶ или присоедините уже запущенный ниже.
    </p>
  {:else}
    <ul class="space-y-2" aria-label="Процессы">
      {#each rows as row (row.id)}
        {@const a = processActions(row)}
        {@const busy = busyId === row.id}
        <li
          class="rounded-md border border-border p-3 data-[selected=true]:border-primary"
          data-selected={row.id === selectedId}
        >
          <div class="flex items-start justify-between gap-2">
            <button
              type="button"
              class="min-w-0 flex-1 text-left"
              aria-pressed={row.id === selectedId}
              onclick={() => onSelect(row.id)}
            >
              <span class="block truncate text-sm font-medium">{row.project}</span>
              <span class="block truncate font-mono text-xs text-muted-foreground" title={row.command}>{row.command}</span>
            </button>
            <Badge variant={row.paused ? "outline" : a.stop ? "default" : "secondary"}>{processStateLabel(row)}</Badge>
          </div>

          <div class="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
            <span>{processOriginLabel(row.origin)}</span>
            {#if row.port != null}
              <span aria-hidden="true">·</span>
              {#if a.open}
                <button
                  type="button"
                  class="inline-flex items-center gap-1 font-mono text-foreground underline-offset-2 hover:underline"
                  onclick={() => onOpen(row.port!)}
                  title="Открыть в браузере"
                >
                  localhost:{row.port}
                  <ExternalLinkIcon class="size-3" aria-hidden="true" />
                </button>
              {:else}
                <span class="font-mono">:{row.port}</span>
              {/if}
            {/if}
          </div>

          <div class="mt-2 flex flex-wrap items-center gap-1">
            {#if a.start}
              <Tooltip.Root>
                <Tooltip.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} size="sm" variant="outline" class="h-7 gap-1 px-2 text-xs" disabled={busy} aria-label="Запустить" onclick={() => onStart(row)}>
                      <PlayIcon class="size-3.5" />
                      Запустить
                    </Button>
                  {/snippet}
                </Tooltip.Trigger>
                <Tooltip.Content side="top">Запустить с той же командой</Tooltip.Content>
              </Tooltip.Root>
            {/if}
            {#if a.pause}
              <Tooltip.Root>
                <Tooltip.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} size="sm" variant="outline" class="h-7 gap-1 px-2 text-xs" disabled={busy} aria-label="Пауза" onclick={() => onPause(row)}>
                      <PauseIcon class="size-3.5" />
                      Пауза
                    </Button>
                  {/snippet}
                </Tooltip.Trigger>
                <Tooltip.Content side="top">Пауза: процесс замрёт, но не остановится</Tooltip.Content>
              </Tooltip.Root>
            {/if}
            {#if a.resume}
              <Tooltip.Root>
                <Tooltip.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} size="sm" variant="outline" class="h-7 gap-1 px-2 text-xs" disabled={busy} aria-label="Продолжить" onclick={() => onResume(row)}>
                      <PlayIcon class="size-3.5" />
                      Продолжить
                    </Button>
                  {/snippet}
                </Tooltip.Trigger>
                <Tooltip.Content side="top">Продолжить после паузы</Tooltip.Content>
              </Tooltip.Root>
            {/if}
            {#if a.stop}
              <Tooltip.Root>
                <Tooltip.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} size="sm" variant="outline" class="h-7 gap-1 px-2 text-xs" disabled={busy} aria-label="Стоп" onclick={() => onStop(row)}>
                      <SquareIcon class="size-3.5" />
                      Стоп
                    </Button>
                  {/snippet}
                </Tooltip.Trigger>
                <Tooltip.Content side="top">Остановить процесс и его группу</Tooltip.Content>
              </Tooltip.Root>
            {/if}
            {#if a.restart}
              <Tooltip.Root>
                <Tooltip.Trigger>
                  {#snippet child({ props })}
                    <Button {...props} size="sm" variant="outline" class="h-7 gap-1 px-2 text-xs" disabled={busy} aria-label="Перезапустить" onclick={() => onRestart(row)}>
                      <RotateCwIcon class="size-3.5" />
                      Перезапуск
                    </Button>
                  {/snippet}
                </Tooltip.Trigger>
                <Tooltip.Content side="top">Остановить и запустить заново</Tooltip.Content>
              </Tooltip.Root>
            {/if}
            <Tooltip.Root>
              <Tooltip.Trigger>
                {#snippet child({ props })}
                  <Button {...props} size="sm" variant="outline" class="h-7 gap-1 px-2 text-xs" aria-label="Логи" onclick={() => onLogs(row)}>
                    <ScrollTextIcon class="size-3.5" />
                    Логи
                  </Button>
                {/snippet}
              </Tooltip.Trigger>
              <Tooltip.Content side="top">Показать логи процесса</Tooltip.Content>
            </Tooltip.Root>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if attachable.length > 0}
    <div class="mt-4 border-t border-border pt-3">
      <p class="mb-2 text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">Не под управлением</p>
      <ul class="space-y-2" aria-label="Присоединяемые процессы">
        {#each attachable as port (`${port.pid}:${port.port}`)}
          <li class="flex items-center justify-between gap-2 rounded-md border border-dashed border-border p-2.5">
            <div class="min-w-0 text-xs">
              <p class="truncate font-medium text-sm" title={port.project ?? undefined}>{shortProjectLabel(port.project)}</p>
              <p class="truncate font-mono text-muted-foreground" title={port.command}>{port.command} · :{port.port}</p>
            </div>
            <Button size="sm" variant="outline" class="shrink-0 gap-1" onclick={() => onAttach(port)}>
              <PlugIcon class="size-3.5" aria-hidden="true" />
              Присоединить
            </Button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</Tooltip.Provider>
