<script lang="ts">
  import type { AgentCatalogEntry, AgentRunner, Project, Task } from "$lib/types";
  import { AGENT_RUNNERS } from "$lib/types";
  import {
    TASK_BOARD_COLUMNS,
    TASK_COLUMN_PREVIEW,
    canOpenTask,
    canStartAgentFromTask,
    countByStatus,
    filterTasksByProject,
    formatTaskBoardSummary,
    tasksForColumn,
    cleanTaskTitle,
    splitColumnPreview,
  } from "$lib/tasks";
  import { agentInstallHint, agentRunnerOptionLabel } from "$lib/agents";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import ExternalLinkIcon from "@lucide/svelte/icons/external-link";
  import WandSparklesIcon from "@lucide/svelte/icons/wand-sparkles";
  import LoaderCircleIcon from "@lucide/svelte/icons/loader-circle";

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

{#snippet taskCard(task: Task)}
  <article
    class="min-w-0 space-y-1 overflow-hidden [overflow-wrap:anywhere] rounded-md border border-border bg-card p-2.5 text-sm"
  >
    <div class="flex min-w-0 items-start gap-1.5">
      <strong class="min-w-0 flex-1 break-words font-medium [overflow-wrap:anywhere]">
        {cleanTaskTitle(task.title)}
      </strong>
      <div class="flex shrink-0 items-center gap-0.5">
        {#if canOpenTask(task)}
          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  type="button"
                  size="sm"
                  variant="outline"
                  class="size-7 p-0"
                  aria-label="Открыть источник"
                  onclick={() => onOpen(task)}
                >
                  <ExternalLinkIcon class="size-3.5" />
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content side="top" portalDisabled>Открыть источник</Tooltip.Content>
          </Tooltip.Root>
        {/if}
        {#if onStartAgent && canStartAgentFromTask(task)}
          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  type="button"
                  size="sm"
                  class="size-7 p-0"
                  disabled={tasksBusy || startBusyTaskId === task.id}
                  aria-label={startBusyTaskId === task.id ? "Запуск…" : "Исправить"}
                  onclick={() => onStartAgent(task)}
                >
                  {#if startBusyTaskId === task.id}
                    <LoaderCircleIcon class="size-3.5 animate-spin" />
                  {:else}
                    <WandSparklesIcon class="size-3.5" />
                  {/if}
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content side="top" portalDisabled
              >{startBusyTaskId === task.id ? "Запуск…" : "Исправить"}</Tooltip.Content
            >
          </Tooltip.Root>
        {/if}
      </div>
    </div>
    <p class="truncate text-xs text-muted-foreground" title={task.project}>{task.project}</p>
    <Badge variant="outline" class="max-w-full truncate">{task.source}</Badge>
  </article>
{/snippet}

<Tooltip.Provider delayDuration={250}>
  <Card.Root
    class="min-w-0 gap-3 overflow-hidden rounded-lg py-3 shadow-none"
    aria-labelledby="tasks-title"
  >
    <Card.Header class="px-4 pb-0">
      <Card.Title
        id="tasks-title"
        class="text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground"
        >Задачи</Card.Title
      >
    </Card.Header>
    <Card.Content class="min-w-0 space-y-3 px-4">
      <div class="flex min-w-0 flex-wrap items-center gap-3 text-sm text-muted-foreground">
        <label class="flex min-w-0 max-w-full items-center gap-2"
          >Проект
          <select
            class="h-9 max-w-full min-w-0 truncate rounded-md border border-input bg-background px-2 text-sm"
            bind:value={projectFilter}
          >
            <option value="">Все проекты</option>
            {#each projects as p (p.path)}
              <option value={p.path} title="{p.name} — {p.path}"
                >{p.name} — {p.path}</option
              >
            {/each}
          </select>
        </label>
        <label class="flex items-center gap-2"
          >Раннер
          <select
            class="h-9 rounded-md border border-input bg-background px-2 text-sm"
            bind:value={agentRunner}
            aria-label="Раннер агента"
          >
            {#each AGENT_RUNNERS as runner (runner)}
              <option value={runner}
                >{agentRunnerOptionLabel(runner, agentCatalog)}</option
              >
            {/each}
          </select>
        </label>
        <Button
          variant="outline"
          disabled={tasksBusy || projects.length === 0}
          onclick={onRefresh}
        >
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
        <p class="text-sm text-muted-foreground">
          Сначала выберите корневые папки и выполните сканирование.
        </p>
      {/if}
      <div
        class="grid min-w-0 grid-cols-1 gap-3 overflow-x-hidden md:grid-cols-2 2xl:grid-cols-4"
      >
        {#each TASK_BOARD_COLUMNS as column (column.status)}
          {@const columnTasks = tasksForColumn(visibleTasks, column.status)}
          {@const split = splitColumnPreview(columnTasks, TASK_COLUMN_PREVIEW)}
          <section
            class="min-w-0 space-y-2 overflow-hidden rounded-md border border-border/80 bg-muted/50 p-2"
            aria-label={column.label}
          >
            <h3
              class="truncate text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground"
            >
              {column.label}
              {#if columnTasks.length > 0}
                <span class="font-normal tabular-nums text-muted-foreground/80"
                  >· {columnTasks.length}</span
                >
              {/if}
            </h3>
            {#if columnTasks.length === 0}
              <p class="text-xs text-muted-foreground">Нет задач</p>
            {:else}
              {#each split.preview as task (task.id)}
                {@render taskCard(task)}
              {/each}
              {#if split.rest.length > 0}
                <details class="group rounded-md border border-border/60 bg-background/40">
                  <summary
                    class="cursor-pointer list-none px-2 py-1.5 text-xs text-muted-foreground marker:content-none hover:text-foreground [&::-webkit-details-marker]:hidden"
                  >
                    <span class="inline-flex items-center gap-1">
                      <span
                        class="inline-block transition-transform group-open:rotate-90"
                        aria-hidden="true">▸</span
                      >
                      Ещё {split.rest.length}
                    </span>
                  </summary>
                  <div class="space-y-2 border-t border-border/60 p-2 pt-2">
                    {#each split.rest as task (task.id)}
                      {@render taskCard(task)}
                    {/each}
                  </div>
                </details>
              {/if}
            {/if}
          </section>
        {/each}
      </div>
    </Card.Content>
  </Card.Root>
</Tooltip.Provider>
