<script lang="ts">
  import type { DashboardSummary } from "$lib/dashboard";
  import type { Snippet } from "svelte";
  import BoxesIcon from "@lucide/svelte/icons/boxes";
  import CircleAlertIcon from "@lucide/svelte/icons/circle-alert";
  import UnplugIcon from "@lucide/svelte/icons/unplug";
  import ListTodoIcon from "@lucide/svelte/icons/list-todo";
  import ClockIcon from "@lucide/svelte/icons/clock";

  interface Props {
    summary: DashboardSummary;
    updatedAt: string;
    /** Панель действий окна (справа). */
    actions?: Snippet;
  }

  let { summary, updatedAt, actions }: Props = $props();
</script>

<div
  class="flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-border px-4 py-2 text-sm text-muted-foreground"
  role="status"
  aria-label="Сводка"
>
  <span class="inline-flex items-center gap-1.5" title="Проектов">
    <BoxesIcon class="size-3.5 shrink-0" aria-hidden="true" />
    <b class="font-medium tabular-nums text-foreground">{summary.projectCount}</b>
    <span class="sr-only">Проектов</span>
  </span>
  <span
    class="inline-flex items-center gap-1.5 {summary.crashedCount > 0 ? 'text-destructive' : ''}"
    title="Упало"
  >
    <CircleAlertIcon class="size-3.5 shrink-0" aria-hidden="true" />
    <b
      class="font-medium tabular-nums {summary.crashedCount > 0
        ? 'text-destructive'
        : 'text-foreground'}">{summary.crashedCount}</b
    >
    <span class="sr-only">Упало</span>
  </span>
  <span
    class="inline-flex items-center gap-1.5 {summary.portConflictCount > 0 ? 'text-primary' : ''}"
    title="Конфликт портов"
  >
    <UnplugIcon class="size-3.5 shrink-0" aria-hidden="true" />
    <b
      class="font-medium tabular-nums {summary.portConflictCount > 0
        ? 'text-primary'
        : 'text-foreground'}">{summary.portConflictCount}</b
    >
    <span class="sr-only">Конфликт портов</span>
  </span>
  <span class="inline-flex items-center gap-1.5" title="Задач">
    <ListTodoIcon class="size-3.5 shrink-0" aria-hidden="true" />
    <b class="font-medium tabular-nums text-foreground">{summary.taskCount}</b>
    <span class="sr-only">Задач</span>
  </span>
  <div class="ml-auto flex flex-wrap items-center gap-2">
    <span class="inline-flex items-center gap-1.5 text-muted-foreground" title="Обновлено">
      <ClockIcon class="size-3.5 shrink-0" aria-hidden="true" />
      <span class="tabular-nums">{updatedAt}</span>
    </span>
    {#if actions}
      {@render actions()}
    {/if}
  </div>
</div>
