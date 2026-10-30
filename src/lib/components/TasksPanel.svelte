<script lang="ts">
  import type { Project, Task } from "$lib/types";
  import {
    TASK_BOARD_COLUMNS,
    canOpenTask,
    filterTasksByProject,
    tasksForColumn,
  } from "$lib/tasks";

  interface Props {
    tasks: Task[];
    taskErrors: string[];
    tasksBusy: boolean;
    projectFilter: string;
    projects: Project[];
    onRefresh: () => void;
    onOpen: (task: Task) => void | Promise<void>;
  }

  let {
    tasks,
    taskErrors,
    tasksBusy,
    projectFilter = $bindable(""),
    projects,
    onRefresh,
    onOpen,
  }: Props = $props();

  const visibleTasks = $derived(filterTasksByProject(tasks, projectFilter));
</script>

<section aria-labelledby="tasks-title">
  <h2 id="tasks-title">Задачи</h2>
  <div class="row">
    <label
      >Проект
      <select bind:value={projectFilter}>
        <option value="">Все проекты</option>
        {#each projects as p (p.path)}
          <option value={p.path}>{p.name} — {p.path}</option>
        {/each}
      </select></label
    >
    <button type="button" disabled={tasksBusy || projects.length === 0} onclick={onRefresh}
      >{tasksBusy ? "Загрузка…" : "Обновить задачи"}</button
    >
  </div>
  {#each taskErrors as message, i (i)}
    <p class="error" role="alert">{message}</p>
  {/each}
  {#if projects.length === 0}
    <p>Сначала выберите корневые папки и выполните сканирование.</p>
  {/if}
  <div class="task-board">
    {#each TASK_BOARD_COLUMNS as column (column.status)}
      <section class="task-column" aria-label={column.label}>
        <h3>{column.label}</h3>
        {#each tasksForColumn(visibleTasks, column.status) as task (task.id)}
          <article class="task-card">
            <strong>{task.title}</strong>
            <p>{task.project}</p>
            <small>{task.source}</small>
            {#if canOpenTask(task)}
              <button type="button" onclick={() => onOpen(task)}>Открыть источник</button>
            {/if}
          </article>
        {:else}
          <p class="empty">Нет задач</p>
        {/each}
      </section>
    {/each}
  </div>
</section>

<style>
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 8px 0;
    flex-wrap: wrap;
  }
  label {
    flex: 1;
    min-width: 240px;
  }
  select {
    max-width: 100%;
    font: inherit;
  }
  .error {
    color: #b00020;
    overflow-wrap: anywhere;
  }
  .task-board {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }
  .task-column {
    background: #f4f5f8;
    border-radius: 8px;
    padding: 12px;
  }
  .task-column h3 {
    margin-top: 0;
    font-size: 1rem;
  }
  .task-card {
    background: white;
    border: 1px solid #ddd;
    border-radius: 6px;
    padding: 10px;
    margin-bottom: 10px;
    overflow-wrap: anywhere;
  }
  .task-card p {
    font-size: 0.8rem;
    color: #555;
  }
  .task-card button {
    display: block;
    margin-top: 8px;
  }
  .empty {
    color: #555;
  }
  @media (max-width: 760px) {
    .task-board {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 440px) {
    .task-board {
      grid-template-columns: 1fr;
    }
  }
</style>
