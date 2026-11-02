<script lang="ts">
  import type { LogLine } from "$lib/types";
  import { Button } from "$lib/components/ui/button/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import XIcon from "@lucide/svelte/icons/x";

  const MIN_W = 220;
  const MAX_W = 720;
  const DEFAULT_W = 320;

  let {
    open = $bindable(false),
    width = $bindable(DEFAULT_W),
    logs = [],
    projectName = null as string | null,
    onReportProblems,
    reportBusy = false,
    reportMessage = "",
  }: {
    open?: boolean;
    width?: number;
    logs?: LogLine[];
    projectName?: string | null;
    /** Создать fix-задачу по ошибкам и предупреждениям лога (#issues). */
    onReportProblems?: () => void | Promise<void>;
    reportBusy?: boolean;
    reportMessage?: string;
  } = $props();

  let dragging = $state(false);

  function clampWidth(n: number): number {
    return Math.min(MAX_W, Math.max(MIN_W, Math.round(n)));
  }

  function onHandlePointerDown(e: PointerEvent) {
    e.preventDefault();
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    dragging = true;
    const startX = e.clientX;
    const startW = width;

    function onMove(ev: PointerEvent) {
      // тянем левый край вправо → уже; влево → шире
      width = clampWidth(startW + (startX - ev.clientX));
    }
    function onUp(ev: PointerEvent) {
      dragging = false;
      handle.releasePointerCapture(ev.pointerId);
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      handle.removeEventListener("pointercancel", onUp);
    }
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
    handle.addEventListener("pointercancel", onUp);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && open) {
      e.preventDefault();
      open = false;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <aside
    class="absolute inset-y-0 right-0 z-40 flex min-h-0 border-l border-border bg-card shadow-[-8px_0_24px_rgba(0,0,0,0.35)]"
    style="width: {clampWidth(width)}px"
    aria-label="Панель логов"
  >
    <button
      type="button"
      class={[
        "group absolute inset-y-0 left-0 z-10 w-1.5 -translate-x-1/2 cursor-col-resize border-0 bg-transparent p-0",
        "hover:bg-primary/40 focus-visible:bg-primary/50 focus-visible:outline-none",
        dragging && "bg-primary/50",
      ]}
      aria-label="Изменить ширину панели логов"
      onpointerdown={onHandlePointerDown}
    >
      <span
        class="absolute left-1/2 top-1/2 size-8 -translate-x-1/2 -translate-y-1/2 rounded-sm opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100"
        aria-hidden="true"
      ></span>
    </button>

    <section class="flex h-full min-w-0 flex-1 flex-col overflow-hidden" aria-labelledby="logs-layer-title">
      <div class="flex items-center gap-2 border-b border-border px-3 py-2">
        <h2
          id="logs-layer-title"
          class="min-w-0 flex-1 truncate font-sans text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground"
        >
          Логи{#if projectName}
            · {projectName}{/if}
        </h2>
        {#if onReportProblems}
          <Button
            size="sm"
            variant="outline"
            class="h-7 shrink-0 px-2 text-xs"
            disabled={reportBusy || logs.length === 0}
            title="Создать fix-задачу по ошибкам и предупреждениям лога и запустить агента, если он доступен"
            onclick={() => onReportProblems?.()}
          >
            {reportBusy ? "Создание…" : "Fix-задача"}
          </Button>
        {/if}
        <Button
          size="sm"
          variant="ghost"
          class="size-7 shrink-0 p-0"
          onclick={() => (open = false)}
          aria-label="Скрыть логи"
          title="Скрыть логи"
        >
          <XIcon class="size-3.5" />
        </Button>
      </div>
      {#if reportMessage}
        <p class="border-b border-border px-3 py-2 text-xs text-muted-foreground" role="status">{reportMessage}</p>
      {/if}
      <ScrollArea class="min-h-0 flex-1 px-1">
        {#if logs.length === 0}
          <p class="px-3 py-4 text-sm text-muted-foreground">Нет строк лога. Запустите процесс.</p>
        {:else}
          <pre
            class="px-3 pb-3 font-mono text-[11.5px] leading-[1.7] text-[#b5a888]">{#each logs as line, i (i)}<span class={line.stream === "stderr" ? "text-primary" : ""}>{line.text}
</span>{/each}</pre>
        {/if}
      </ScrollArea>
    </section>
  </aside>
{/if}
