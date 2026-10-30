<script lang="ts">
  import type { AgentRunner, Project, Task } from "$lib/types";
  import { AGENT_RUNNERS } from "$lib/types";
  import {
    TASK_BOARD_COLUMNS,
    canOpenTask,
    canStartAgentFromTask,
    filterTasksByProject,
    tasksForColumn,
  } from "$lib/tasks";
  import { agentRunnerLabel } from "$lib/agents";

  interface Props {
    tasks: Task[];
    taskErrors: string[];
    tasksBusy: boolean;
    projectFilter: string;
    projects: Project[];
    /** Раннер для «Исправить» (#30); Claude по умолчанию. */
    agentRunner?: AgentRunner;
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
    startBusyTaskId = null,
    onRefresh,
    onOpen,
    onStartAgent,
  }: Props = $props();

  const visibleTasks = $derived(filterTasksByProject(tasks, projectFilter));
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
</script>

<Card.Root aria-labelledby="tasks-title">
  <Card.Header>
    <Card.Title id="tasks-title" class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Задачи</Card.Title>
  </Card.Header>
  <Card.Content class="space-y-3">
    <div class="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
      <label class="flex items-center gap-2">Проект
        <select class="h-9 rounded-md border border-input bg-background px-2 text-sm" bind:value={projectFilter}>
          <option value="">Все проекты</option>
          {#each projects as p (p.path)}
            <option value={p.path}>{p.name} — {p.path}</option>
          {/each}
        </select>
      </label>
      <label class="flex items-center gap-2">Раннер
        <select class="h-9 rounded-md border border-input bg-background px-2 text-sm" bind:value={agentRunner} aria-label="Раннер агента">
          {#each AGENT_RUNNERS as runner (runner)}
            <option value={runner}>{agentRunnerLabel(runner)}</option>
          {/each}
        </select>
      </label>
      <Button variant="outline" disabled={tasksBusy || projects.length === 0} onclick={onRefresh}>
        {tasksBusy ? "Загрузка…" : "Обновить задачи"}
      </Button>
    </div>
    {#each taskErrors as message, i (i)}
      <p class="break-words text-sm text-destructive" role="alert">{message}</p>
    {/each}
    {#if projects.length === 0}
      <p class="text-sm text-muted-foreground">Сначала выберите корневые папки и выполните сканирование.</p>
    {/if}
    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {#each TASK_BOARD_COLUMNS as column (column.status)}
        <section class="space-y-2 rounded-lg bg-muted p-2" aria-label={column.label}>
          <h3 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">{column.label}</h3>
          {#each tasksForColumn(visibleTasks, column.status) as task (task.id)}
            <article class="space-y-1 rounded-md border bg-card p-3 text-sm">
              <strong class="font-medium">{task.title}</strong>
              <p class="text-xs text-muted-foreground">{task.project}</p>
              <Badge variant="outline">{task.source}</Badge>
              <div class="flex flex-wrap gap-2 pt-1">
                {#if canOpenTask(task)}
                  <Button size="sm" variant="outline" onclick={() => onOpen(task)}>Открыть источник</Button>
                {/if}
                {#if onStartAgent && canStartAgentFromTask(task)}
                  <Button size="sm" disabled={tasksBusy || startBusyTaskId === task.id} onclick={() => onStartAgent(task)}>
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
