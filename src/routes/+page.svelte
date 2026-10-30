<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { openPath, openUrl } from "@tauri-apps/plugin-opener";
  import type {
    AcceptAgentRunResult,
    AgentRunRecord,
    AgentRunner,
    Settings,
    PortRow,
    StartAgentRunResult,
    Task,
  } from "$lib/types";
  import { DEFAULT_AGENT_RUNNER, parseAgentRunner } from "$lib/types";
  import {
    isRunning,
    statusLabel,
    type LogLine,
    type Project,
    type ProcessRow,
  } from "$lib/types";
  import { OPEN_PROJECT_EVENT, processIdForProject } from "$lib/alerts";
  import { clampPortPollInterval } from "$lib/ports";
  import { canOpenTask, canStartAgentFromTask } from "$lib/tasks";
  import PortsPanel from "$lib/components/PortsPanel.svelte";
  import TasksPanel from "$lib/components/TasksPanel.svelte";
  import AgentRunsPanel from "$lib/components/AgentRunsPanel.svelte";

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
  let agentRuns = $state<AgentRunRecord[]>([]);
  let agentRunsError = $state("");
  let agentRunsBusy = $state(false);
  let selectedAgentRunId = $state<string | null>(null);
  let agentMergeCommand = $state("");
  let startBusyTaskId = $state<string | null>(null);
  /** Раннер для «Исправить» (#30); Claude — основной путь. */
  let agentRunner = $state<AgentRunner>(DEFAULT_AGENT_RUNNER);
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

  async function startAgentFromTask(task: Task) {
    if (!canStartAgentFromTask(task) || startBusyTaskId) return;
    startBusyTaskId = task.id;
    try {
      const result = await invoke<StartAgentRunResult>("start_agent_from_task", {
        task,
        runner: agentRunner,
      });
      tasks = tasks.map((t) => (t.id === result.task.id ? result.task : t));
      if (result.queued && result.queue_reason) {
        taskErrors = [...taskErrors, `Агент в очереди: ${result.queue_reason}`];
      }
      selectedAgentRunId = result.run.id;
      await refreshAgentRuns();
      // Раннер может ещё работать в фоне — подтянуть статус через пару секунд.
      if (!result.queued) {
        setTimeout(() => {
          void refreshAgentRuns();
        }, 2000);
      }
    } catch (e) {
      taskErrors = [...taskErrors, String(e)];
    } finally {
      startBusyTaskId = null;
    }
  }

  async function refreshAgentRuns() {
    if (!mounted || agentRunsBusy) return;
    agentRunsBusy = true;
    try {
      const result = await invoke<AgentRunRecord[]>("list_agent_runs");
      if (!mounted) return;
      agentRuns = result;
      agentRunsError = "";
      if (selectedAgentRunId && !result.some((r) => r.id === selectedAgentRunId)) {
        selectedAgentRunId = null;
      }
    } catch (e) {
      if (mounted) agentRunsError = String(e);
    } finally {
      agentRunsBusy = false;
    }
  }

  async function acceptAgentRun(id: string) {
    try {
      const result = await invoke<AcceptAgentRunResult>("accept_agent_run", { id });
      agentMergeCommand = result.merge_command;
      await refreshAgentRuns();
    } catch (e) {
      agentRunsError = String(e);
    }
  }

  async function discardAgentRun(id: string) {
    try {
      await invoke("discard_agent_run", { id });
      if (selectedAgentRunId === id) selectedAgentRunId = null;
      if (agentMergeCommand) agentMergeCommand = "";
      await refreshAgentRuns();
    } catch (e) {
      agentRunsError = String(e);
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
        agentRunner = parseAgentRunner(s.agents?.runner);
        if (roots) await scan();
      } catch (e) { if (mounted) error = String(e); }
    })();
    void refresh();
    void refreshPorts();
    void refreshAgentRuns();
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

  type ViewMode = "columns" | "tabs";
  type Tab = "projects" | "processes" | "ports" | "tasks" | "agents" | "logs";
  const TABS: [Tab, string][] = [
    ["projects", "Проекты"],
    ["processes", "Процессы"],
    ["ports", "Порты"],
    ["tasks", "Задачи"],
    ["agents", "Агенты"],
    ["logs", "Логи"],
  ];
  function readPref(key: string): string | null {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  }
  let view = $state<ViewMode>(readPref("demerzel.view") === "tabs" ? "tabs" : "columns");
  let tab = $state<Tab>("projects");
  function setView(v: ViewMode) {
    view = v;
    try {
      localStorage.setItem("demerzel.view", v);
    } catch {
      /* без хранилища режим не запоминается */
    }
  }
  const shown = (t: Tab) => view === "columns" || tab === t;
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

<main class={view}>
  <header>
    <h1>DEMERZEL</h1>
    <div class="seg" role="group" aria-label="Вид окна">
      <button type="button" class:on={view === "columns"} onclick={() => setView("columns")}>Колонки</button>
      <button type="button" class:on={view === "tabs"} onclick={() => setView("tabs")}>Вкладки</button>
    </div>
    {#if focusProject}
      <p class="notify-focus" role="status">Открыто из уведомления: {focusProject}</p>
    {/if}
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if view === "tabs"}
    <nav class="tabs" aria-label="Разделы">
      {#each TABS as [id, label] (id)}
        <button type="button" class:on={tab === id} onclick={() => (tab = id)}>{label}</button>
      {/each}
    </nav>
  {/if}

  <div class="layout">
  {#if shown("projects")}
  <section class="area-projects">
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
  {/if}

  <div class="area-center">
  {#if shown("processes")}
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
  {/if}
  {#if shown("ports")}
  <PortsPanel
    {ports}
    {portsError}
    {portsBusy}
    bind:portInterval
    onRefresh={refreshPorts}
    onStopManaged={stopManagedFromPorts}
  />
  {/if}

  {#if shown("tasks")}
  <TasksPanel
    {tasks}
    {taskErrors}
    {tasksBusy}
    bind:projectFilter
    bind:agentRunner
    projects={flatProjects}
    {startBusyTaskId}
    onRefresh={refreshTasks}
    onOpen={openTask}
    onStartAgent={startAgentFromTask}
  />
  {/if}

  {#if shown("agents")}
  <AgentRunsPanel
    runs={agentRuns}
    runsError={agentRunsError}
    runsBusy={agentRunsBusy}
    bind:selectedId={selectedAgentRunId}
    mergeCommand={agentMergeCommand}
    onRefresh={refreshAgentRuns}
    onSelect={(id) => {
      selectedAgentRunId = id;
    }}
    onAccept={acceptAgentRun}
    onDiscard={discardAgentRun}
  />
  {/if}
  </div>

  {#if shown("logs")}
  <section class="area-logs">
    <h2>Логи {selectedId !== null ? `#${selectedId}` : ""}</h2>
    <pre class="logs">{#each logs as line, i (i)}<span class={{ err: line.stream === "stderr" }}>{line.text}
</span>{/each}</pre>
  </section>
  {/if}
  </div>
</main>

<style>
  :global(body) { margin: 0; background: #14110d; }
  main {
    --bg: #14110d; --pn: #1d1812; --ln: #3a2f1f; --tx: #efe3c8; --mu: #8f8266; --gold: #e0a83a; --red: #d9553a;
    font-family: system-ui, sans-serif; padding: 16px; min-height: 100vh; box-sizing: border-box;
    background: var(--bg); color: var(--tx);
  }
  main.tabs { max-width: 960px; margin: 0 auto; }
  header { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  h1 { margin: 0; font-size: 1.1rem; font-weight: 500; letter-spacing: 0.08em; color: var(--gold); }
  h2 { font-size: 0.7rem; font-weight: 500; text-transform: uppercase; letter-spacing: 0.08em; color: var(--mu); }
  .seg { margin-left: auto; display: flex; border: 1px solid var(--ln); border-radius: 8px; overflow: hidden; }
  .seg button, .tabs button { border: 0; background: transparent; color: var(--mu); padding: 5px 12px; cursor: pointer; font: inherit; }
  .seg button.on, .tabs button.on { background: var(--gold); color: #241a08; font-weight: 500; }
  .tabs { display: flex; gap: 6px; margin: 12px 0; flex-wrap: wrap; }
  .tabs button { border: 1px solid var(--ln); border-radius: 7px; }
  .layout { display: block; }
  main.columns .layout { display: grid; grid-template-columns: 260px minmax(0, 1fr) 320px; gap: 0; margin-top: 12px; border: 1px solid var(--ln); border-radius: 12px; overflow: hidden; background: var(--pn); }
  main.columns .layout > * { padding: 12px 16px; margin: 0; min-width: 0; }
  main.columns .area-projects { border-right: 1px solid var(--ln); }
  main.columns .area-logs { border-left: 1px solid var(--ln); }
  section { margin-bottom: 24px; }
  .row { display: flex; gap: 8px; align-items: center; margin: 8px 0; flex-wrap: wrap; }
  textarea, input { width: 100%; font: inherit; background: var(--bg); color: var(--tx); border: 1px solid var(--ln); border-radius: 6px; box-sizing: border-box; }
  label { flex: 1; min-width: 240px; color: var(--mu); }
  main :global(button) { font: inherit; }
  .tree { list-style: none; padding: 0; }
  .tree li { display: flex; gap: 8px; align-items: center; padding: 4px 0; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--ln); }
  tr.selected { background: #2a2216; }
  .focus-project { background: #2a2216; outline: 1px solid var(--gold); }
  .notify-focus { margin: 0.25rem 0 0; font-size: 0.9rem; color: var(--gold); }
  .logs { background: #0d0b08; color: #b5a888; padding: 12px; min-height: 120px; max-height: 480px; overflow: auto; font-size: 11.5px; }
  .err { color: #ff7b72; }
  .error { color: var(--red); overflow-wrap: anywhere; }
</style>
