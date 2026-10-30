<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { openPath, openUrl } from "@tauri-apps/plugin-opener";
  import type { Settings, PortRow, Task } from "$lib/types";
  import {
    isRunning,
    statusLabel,
    type LogLine,
    type Project,
    type ProcessRow,
  } from "$lib/types";
  import { OPEN_PROJECT_EVENT, processIdForProject } from "$lib/alerts";
  import { clampPortPollInterval } from "$lib/ports";
  import { canOpenTask } from "$lib/tasks";
  import PortsPanel from "$lib/components/PortsPanel.svelte";
  import TasksPanel from "$lib/components/TasksPanel.svelte";

  let roots = $state("");
  let projects = $state<Project[]>([]);
  let processes = $state<ProcessRow[]>([]);
  let selectedId = $state<number | null>(null);
  /** Проект из клика по системному уведомлению (#22). */
  let focusProject = $state<string | null>(null);
  let logs = $state<LogLine[]>([]);
  /** Переопределение: если пусто — берём commands.dev / scripts.dev с диска. */
  let commandOverride = $state("");
  let error = $state("");

  let ports = $state<PortRow[]>([]);
  let portsError = $state("");
  let portsBusy = $state(false);
  let portInterval = $state(5);
  let tasks = $state<Task[]>([]);
  let taskErrors = $state<string[]>([]);
  let tasksBusy = $state(false);
  let projectFilter = $state("");
  let mounted = false;
  let processBusy = false;
  let scanBusy = $state(false);
  let taskGeneration = 0;
  const flatProjects = $derived(flatten(projects));

  function flatten(list: Project[]): Project[] {
    return [...new Map(list.flatMap((p) => [p, ...flatten(p.children)]).map((p) => [p.path, p])).values()];
  }

  async function refreshPorts() {
    if (!mounted || portsBusy) return;
    portsBusy = true;
    try {
      const result = await invoke<PortRow[]>("list_ports");
      if (mounted) { ports = result; portsError = ""; }
    } catch (e) {
      if (mounted) portsError = String(e);
    } finally { portsBusy = false; }
  }

  async function refreshTasks() {
    const generation = ++taskGeneration;
    tasksBusy = true;
    const results = await Promise.all(flatProjects.map(async (p) => {
      try {
        return { tasks: await invoke<Task[]>("project_tasks", { path: p.path }), error: "" };
      } catch (e) { return { tasks: [] as Task[], error: `${p.name} (${p.path}): ${String(e)}` }; }
    }));
    if (!mounted || generation !== taskGeneration) return;
    tasks = results.flatMap((result) => result.tasks);
    taskErrors = results.map((result) => result.error).filter(Boolean);
    tasksBusy = false;
  }

  async function openTask(task: Task) {
    if (!canOpenTask(task) || !task.url) return;
    try {
      if (/^https?:\/\//.test(task.url)) await openUrl(task.url);
      else await openPath(task.url);
    } catch (e) {
      taskErrors = [...taskErrors, String(e)];
    }
  }

  async function stopManagedFromPorts(managedId: number) {
    await stop(managedId);
    await refreshPorts();
  }

  function rootList(): string[] {
    return roots
      .split("\n")
      .map((r) => r.trim())
      .filter(Boolean);
  }

  async function saveRoots() {
    try {
      await invoke("save_settings", { settings: { scan_roots: rootList() } as Settings });
    } catch (e) {
      error = String(e);
    }
  }

  async function scan() {
    error = "";
    if (scanBusy) return;
    scanBusy = true;
    await saveRoots();
    try {
      const found = await invoke<Project[]>("scan_projects", { roots: rootList() });
      if (!mounted) return;
      projects = found;
      if (!flatProjects.some((p) => p.path === projectFilter)) projectFilter = "";
      await refreshTasks();
    } catch (e) {
      error = String(e);
    } finally { scanBusy = false; }
  }

  async function refresh() {
    if (!mounted || processBusy) return;
    processBusy = true;
    try {
      processes = await invoke<ProcessRow[]>("list_processes");
      if (selectedId !== null) {
        const l = await invoke<LogLine[] | null>("process_logs", { id: selectedId });
        logs = l ?? [];
      }
    } catch (e) {
      error = String(e);
    } finally { processBusy = false; }
  }

  async function start(p: Project) {
    error = "";
    try {
      let command = commandOverride.trim();
      // «bun run dev» — старый дефолт UI; для parser/корня без scripts.dev это ломает старт.
      if (!command || command === "bun run dev") {
        const resolved = await invoke<string | null>("resolve_dev_command", { path: p.path });
        if (resolved) {
          command = resolved;
        } else if (!command) {
          error = `У «${p.name}» нет commands.dev / scripts.dev. Укажите команду в поле выше (например bun run pipeline) или добавьте orchestrator.json.`;
          return;
        }
      }
      const id = await invoke<number>("start_process", {
        project: p.name,
        path: p.path,
        command,
      });
      selectedId = id;
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function stop(id: number) {
    try {
      await invoke("stop_process", { id });
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  function select(id: number) {
    selectedId = id;
    refresh();
  }

  function openProjectFromNotify(project: string) {
    focusProject = project;
    const id = processIdForProject(processes, project);
    if (id !== null) select(id);
    const match = flatProjects.find((p) => p.name === project);
    if (match) projectFilter = match.path;
  }

  onMount(() => {
    mounted = true;
    let unlistenOpen: (() => void) | undefined;
    void (async () => {
      try {
        unlistenOpen = await listen<{ project: string }>(OPEN_PROJECT_EVENT, (ev) => {
          if (!mounted) return;
          openProjectFromNotify(ev.payload.project);
        });
      } catch (e) {
        if (mounted) error = String(e);
      }
      try {
        const s = await invoke<Settings>("get_settings");
        if (!mounted) return;
        roots = s.scan_roots.join("\n");
        if (roots) await scan();
      } catch (e) { if (mounted) error = String(e); }
    })();
    void refresh();
    void refreshPorts();
    const poll = setInterval(() => { void refresh(); }, 1000);
    let nextPortPoll = Date.now();
    const portPoll = setInterval(() => {
      const interval = clampPortPollInterval(Number(portInterval));
      if (Date.now() >= nextPortPoll) {
        nextPortPoll = Date.now() + interval * 1000;
        void refreshPorts();
      }
    }, 1000);
    return () => {
      mounted = false;
      taskGeneration++;
      clearInterval(poll);
      clearInterval(portPoll);
      unlistenOpen?.();
    };
  });
</script>

{#snippet tree(list: Project[], depth: number)}
  {#each list as p (p.path)}
    <li style="padding-left: {depth * 16}px" class={{ 'focus-project': focusProject === p.name }}>
      <span>{p.name}</span>
      <button type="button" onclick={() => start(p)}>запустить</button>
    </li>
    {#if p.children.length}
      {@render tree(p.children, depth + 1)}
    {/if}
  {/each}
{/snippet}

<main>
  <header>
    <h1>Demerzel</h1>
    {#if focusProject}
      <p class="notify-focus" role="status">Открыто из уведомления: {focusProject}</p>
    {/if}
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <section>
    <h2>Корневые папки</h2>
    <textarea aria-label="Корневые папки сканирования" bind:value={roots} rows="2" placeholder="/Users/you/code"></textarea>
    <div class="row">
      <button type="button" onclick={scan} disabled={scanBusy}>Сканировать</button>
      <label
        >команда (пусто = из orchestrator.json / package.json):
        <input bind:value={commandOverride} placeholder="авто" />
      </label>
    </div>
    <ul class="tree">
      {@render tree(projects, 0)}
    </ul>
  </section>

  <section>
    <h2>Процессы</h2>
    {#if processes.length === 0}
      <p>Нет запущенных процессов.</p>
    {:else}
      <table>
        <thead>
          <tr><th>проект</th><th>команда</th><th>статус</th><th></th></tr>
        </thead>
        <tbody>
          {#each processes as row (row.id)}
            <tr class={{ selected: row.id === selectedId }}>
              <td><button type="button" onclick={() => select(row.id)} aria-pressed={row.id === selectedId}>{row.project}</button></td>
              <td>{row.command}</td>
              <td>{statusLabel(row.status)}</td>
              <td>
                {#if isRunning(row.status)}
                  <button
                    type="button"
                    onclick={(e) => {
                      e.stopPropagation();
                      stop(row.id);
                    }}>стоп</button
                  >
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <section>
    <h2>Логи {selectedId !== null ? `#${selectedId}` : ""}</h2>
    <pre class="logs">{#each logs as line, i (i)}<span class={{ err: line.stream === "stderr" }}>{line.text}
</span>{/each}</pre>
  </section>
  <PortsPanel
    {ports}
    {portsError}
    {portsBusy}
    bind:portInterval
    onRefresh={refreshPorts}
    onStopManaged={stopManagedFromPorts}
  />

  <TasksPanel
    {tasks}
    {taskErrors}
    {tasksBusy}
    bind:projectFilter
    projects={flatProjects}
    onRefresh={refreshTasks}
    onOpen={openTask}
  />
</main>

<style>
  main { font-family: system-ui, sans-serif; padding: 16px; max-width: 960px; margin: 0 auto; }
  section { margin-bottom: 24px; }
  .row { display: flex; gap: 8px; align-items: center; margin: 8px 0; flex-wrap: wrap; }
  textarea, input { width: 100%; font: inherit; }
  label { flex: 1; min-width: 240px; }
  .tree { list-style: none; padding: 0; }
  .tree li { display: flex; gap: 8px; align-items: center; padding: 4px 0; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid #ddd; }
  tr.selected { background: #f0f0ff; }
  .focus-project { background: #f5f0e6; outline: 1px solid #c4a574; }
  .notify-focus { margin: 0.25rem 0 0; font-size: 0.9rem; color: #5c4a32; }
  .logs { background: #0d0d12; color: #ddd; padding: 12px; min-height: 120px; max-height: 320px; overflow: auto; }
  .err { color: #ff7b72; }
  .error { color: #b00020; overflow-wrap: anywhere; }
</style>
