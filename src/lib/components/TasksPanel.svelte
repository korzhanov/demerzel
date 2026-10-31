<script lang="ts">
  import type { AgentCatalogEntry, AgentRunner, Project, Task } from "$lib/types";
  import { AGENT_RUNNERS } from "$lib/types";
  import {
    TASK_BOARD_COLUMNS,
    canOpenTask,
    canStartAgentFromTask,
    countByStatus,
    filterTasksByProject,
    formatTaskBoardSummary,
    tasksForColumn,
  } from "$lib/tasks";
  import { agentInstallHint, agentRunnerOptionLabel } from "$lib/agents";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";

  interface Props {
    tasks: Task[];
    taskErrors: string[];
    tasksBusy: boolean;
    projectFilter: string;
    projects: Project[];
    /** Раннер для «Исправить» (#30 / #95); Claude по умолчанию. */
    agentRunner?: AgentRunner;
    /** Реестр установленных CLI (#95). */
    agentCatalog?: AgentCatalogEntry[];
    startBusyTaskId?: string | null;
    onRefresh: () => void;
    onOpen: (task: Task) => void | Promise<void>;
    onStartAgent?: (task: Task) => void | Promise<void>;
  }

  let {
    tasks,
    taskErrors,
    tasksBusy,
    projectFilter = $bindable(""),
    projects,
    agentRunner = $bindable("claude"),
    agentCatalog = [],
    startBusyTaskId = null,
    onRefresh,
    onOpen,
    onStartAgent,
  }: Props = $props();

  const visibleTasks = $derived(filterTasksByProject(tasks, projectFilter));
  const boardSummary = $derived(formatTaskBoardSummary(countByStatus(visibleTasks)));
  const runnerHint = $derived(agentInstallHint(agentRunner, agentCatalog));
</script>

<Card.Root class="min-w-0 overflow-hidden" aria-labelledby="tasks-title">
  <Card.Header>
    <Card.Title id="tasks-title" class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Задачи</Card.Title>
  </Card.Header>
  <Card.Content class="min-w-0 space-y-3">
    <div class="flex min-w-0 flex-wrap items-center gap-3 text-sm text-muted-foreground">
      <label class="flex min-w-0 max-w-full items-center gap-2">Проект
        <select class="h-9 max-w-full min-w-0 truncate rounded-md border border-input bg-background px-2 text-sm" bind:value={projectFilter}>
          <option value="">Все проекты</option>
          {#each projects as p (p.path)}
            <option value={p.path} title="{p.name} — {p.path}">{p.name} — {p.path}</option>
          {/each}
        </select>
      </label>
      <label class="flex items-center gap-2">Раннер
        <select class="h-9 rounded-md border border-input bg-background px-2 text-sm" bind:value={agentRunner} aria-label="Раннер агента">
          {#each AGENT_RUNNERS as runner (runner)}
            <option value={runner}>{agentRunnerOptionLabel(runner, agentCatalog)}</option>
          {/each}
        </select>
      </label>
      <Button variant="outline" disabled={tasksBusy || projects.length === 0} onclick={onRefresh}>
        {tasksBusy ? "Загрузка…" : "Обновить задачи"}
      </Button>
    </div>
    {#if runnerHint}
      <p class="text-xs text-muted-foreground" role="note">{runnerHint}</p>
    {/if}
    {#if projects.length > 0}
      <p class="text-sm text-muted-foreground" aria-live="polite">{boardSummary}</p>
    {/if}
    {#each taskErrors as message, i (i)}
      <p class="break-words text-sm text-destructive" role="alert">{message}</p>
    {/each}
    {#if projects.length === 0}
      <p class="text-sm text-muted-foreground">Сначала выберите корневые папки и выполните сканирование.</p>
    {/if}
    <div class="grid min-w-0 grid-cols-1 gap-3 overflow-x-hidden sm:grid-cols-2 xl:grid-cols-4">
      {#each TASK_BOARD_COLUMNS as column (column.status)}
        <section class="min-w-0 space-y-2 overflow-hidden rounded-lg bg-muted p-2" aria-label={column.label}>
          <h3 class="truncate text-xs font-medium uppercase tracking-wider text-muted-foreground">{column.label}</h3>
          {#each tasksForColumn(visibleTasks, column.status) as task (task.id)}
            <article class="min-w-0 space-y-1 overflow-hidden rounded-md border bg-card p-3 text-sm">
              <strong class="block break-words font-medium [overflow-wrap:anywhere]">{task.title}</strong>
              <p class="truncate text-xs text-muted-foreground" title={task.project}>{task.project}</p>
              <Badge variant="outline" class="max-w-full truncate">{task.source}</Badge>
              <div class="flex min-w-0 flex-wrap gap-2 pt-1">
                {#if canOpenTask(task)}
                  <Button size="sm" variant="outline" class="max-w-full shrink" onclick={() => onOpen(task)}>Открыть источник</Button>
                {/if}
                {#if onStartAgent && canStartAgentFromTask(task)}
                  <Button size="sm" class="shrink" disabled={tasksBusy || startBusyTaskId === task.id} onclick={() => onStartAgent(task)}>
                    {startBusyTaskId === task.id ? "Запуск…" : "Исправить"}
                  </Button>
                {/if}
              </div>
            </article>
          {:else}
            <p class="text-xs text-muted-foreground">Нет задач</p>
          {/each}
        </section>
      {/each}
    </div>
  </Card.Content>
</Card.Root>
