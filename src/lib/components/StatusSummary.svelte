<script lang="ts">
  import type { DashboardSummary } from "$lib/dashboard";

  interface Props {
    summary: DashboardSummary;
    updatedAt: string;
  }

  let { summary, updatedAt }: Props = $props();
</script>

<div
  class="flex flex-wrap items-center gap-x-5 gap-y-1 border-b border-border px-4 py-2.5 text-sm text-muted-foreground"
  role="status"
  aria-label="Сводка"
>
  <span>Проектов <b class="font-medium text-foreground">{summary.projectCount}</b></span>
  <span class={summary.crashedCount > 0 ? "text-destructive" : ""}>
    Упало <b class="font-medium {summary.crashedCount > 0 ? 'text-destructive' : 'text-foreground'}">{summary.crashedCount}</b>
  </span>
  <span class={summary.portConflictCount > 0 ? "text-primary" : ""}>
    Конфликт портов <b class="font-medium {summary.portConflictCount > 0 ? 'text-primary' : 'text-foreground'}">{summary.portConflictCount}</b>
  </span>
  <span>Задач <b class="font-medium text-foreground">{summary.taskCount}</b></span>
  <span class="ml-auto text-muted-foreground">обновлено {updatedAt}</span>
</div>
