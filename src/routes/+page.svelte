<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { openPath, openUrl } from "@tauri-apps/plugin-opener";
  import type {
    AcceptAgentRunResult,
    AgentCatalogEntry,
    AgentRunRecord,
    AgentRunner,
    AutofixMode,
    AutofixOffer,
    Settings,
    PortRow,
    StartAgentFromAlertResult,
    StartAgentRunResult,
    Task,
  } from "$lib/types";
  import {
    AUTOFIX_MODES,
    DEFAULT_AGENT_RUNNER,
    DEFAULT_AUTOFIX_MODE,
    parseAgentRunner,
    parseAutofixMode,
  } from "$lib/types";
  import {
    isRunning,
    statusLabel,
    type LogLine,
    type Project,
    type ProcessRow,
  } from "$lib/types";
  import { AUTOFIX_OFFER_EVENT, OPEN_PROJECT_EVENT, processIdForProject } from "$lib/alerts";
  import { autofixModeLabel } from "$lib/agents";
  import { clampPortPollInterval } from "$lib/ports";
  import { canOpenTask, canStartAgentFromTask } from "$lib/tasks";
  import PortsPanel from "$lib/components/PortsPanel.svelte";
  import TasksPanel from "$lib/components/TasksPanel.svelte";
  import AgentRunsPanel from "$lib/components/AgentRunsPanel.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import * as ToggleGroup from "$lib/components/ui/toggle-group/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import * as Resizable from "$lib/components/ui/resizable/index.js";

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
  /** Раннер для «Исправить» (#30 / #95); Claude — основной путь. */
  let agentRunner = $state<AgentRunner>(DEFAULT_AGENT_RUNNER);
  /** Реестр CLI на машине (#95). */
  let agentCatalog = $state<AgentCatalogEntry[]>([]);
  let settingsAgentRunner = $state<AgentRunner>(DEFAULT_AGENT_RUNNER);
  /** Режим автофикса из settings (#96). */
  let settingsAutofix = $state<AutofixMode>(DEFAULT_AUTOFIX_MODE);
  /** Предложение «исправить» из алерта (#96). */
  let autofixOffer = $state<AutofixOffer | null>(null);
  let autofixBusy = $state(false);
  let mounted = false;
  let processBusy = false;
  let scanBusy = $state(false);
  let taskGeneration = 0;
  const flatProjects = $derived(flatten(projects));

  function flatten(list: Project[]): Project[] {
    return [...new Map(list.flatMap((p) => [p, ...flatten(p.children)]).map((p) => [p.path, p])).values()];
  }

  /** Раннер проекта из orchestrator.json, иначе settings (#95). */
  $effect(() => {
    const path = projectFilter;
    if (!mounted) return;
    if (!path) {
      agentRunner = settingsAgentRunner;
      return;
    }
    void (async () => {
      try {
        const raw = await invoke<string>("resolve_project_agent_runner", {
          projectPath: path,
        });
        if (mounted && projectFilter === path) {
          agentRunner = parseAgentRunner(raw);
        }
      } catch {
        if (mounted && projectFilter === path) {
          agentRunner = settingsAgentRunner;
        }
      }
    })();
  });

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
      await invoke("save_settings", {
        settings: {
          scan_roots: rootList(),
          agents: { runner: settingsAgentRunner, autofix: settingsAutofix },
        } as Settings,
      });
    } catch (e) {
      error = String(e);
    }
  }

  async function saveAutofixMode(mode: AutofixMode) {
    settingsAutofix = mode;
    try {
      const current = await invoke<Settings>("get_settings");
      await invoke("save_settings", {
        settings: {
          ...current,
          scan_roots: rootList().length ? rootList() : current.scan_roots,
          agents: {
            ...(current.agents ?? {}),
            runner: settingsAgentRunner,
            autofix: mode,
          },
        } as Settings,
      });
    } catch (e) {
      error = String(e);
    }
  }

  async function confirmAutofixOffer() {
    const offer = autofixOffer;
    if (!offer || autofixBusy) return;
    autofixBusy = true;
    error = "";
    try {
      const result = await invoke<StartAgentFromAlertResult>("start_agent_from_alert", {
        projectPath: offer.project_path,
        prompt: offer.prompt,
        runner: agentRunner,
      });
      autofixOffer = null;
      if (result.queued) {
        agentRunsError = result.queue_reason ?? "Агент в очереди (лимит / память).";
      }
      await refreshAgentRuns();
      selectedAgentRunId = result.run.id;
      tab = "agents";
    } catch (e) {
      error = String(e);
    } finally {
      autofixBusy = false;
    }
  }

  function dismissAutofixOffer() {
    autofixOffer = null;
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
    let unlistenAutofix: (() => void) | undefined;
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
        unlistenAutofix = await listen<AutofixOffer>(AUTOFIX_OFFER_EVENT, (ev) => {
          if (!mounted) return;
          autofixOffer = ev.payload;
          openProjectFromNotify(ev.payload.project);
        });
      } catch (e) {
        if (mounted) error = String(e);
      }
      try {
        const s = await invoke<Settings>("get_settings");
        if (!mounted) return;
        roots = s.scan_roots.join("\n");
        settingsAgentRunner = parseAgentRunner(s.agents?.runner);
        agentRunner = settingsAgentRunner;
        settingsAutofix = parseAutofixMode(s.agents?.autofix);
        if (roots) await scan();
      } catch (e) { if (mounted) error = String(e); }
      try {
        const catalog = await invoke<AgentCatalogEntry[]>("list_available_agents");
        if (mounted) agentCatalog = catalog;
      } catch (e) {
        if (mounted) taskErrors = [...taskErrors, String(e)];
      }
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
      unlistenAutofix?.();
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
</script>

{#snippet tree(list: Project[], depth: number)}
  {#each list as p (p.path)}
    <li
      style="padding-left: {depth * 12}px"
      class={[
        "flex min-w-0 items-center gap-2 rounded-md px-2 py-1 hover:bg-accent",
        focusProject === p.name && "bg-accent ring-1 ring-primary",
      ]}
      title="{p.name} — {p.path}"
    >
      <span class="min-w-0 flex-1 truncate text-sm">{p.name}</span>
      <Button size="sm" variant="outline" class="shrink-0" onclick={() => start(p)}>Запустить</Button>
    </li>
    {#if p.children.length}
      {@render tree(p.children, depth + 1)}
    {/if}
  {/each}
{/snippet}

{#snippet projectsView()}
  <Card.Root class="h-full min-w-0 overflow-hidden">
    <Card.Header class="px-4">
      <Card.Title class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Проекты</Card.Title>
    </Card.Header>
    <Card.Content class="min-w-0 space-y-3 overflow-x-hidden px-4">
      <Textarea aria-label="Корневые папки сканирования" bind:value={roots} rows={2} placeholder="/Users/you/code" class="min-w-0" />
      <div class="flex min-w-0 flex-wrap items-center gap-2">
        <Button class="shrink-0" onclick={scan} disabled={scanBusy}>Сканировать</Button>
        <Input class="min-w-0 flex-1 basis-32" bind:value={commandOverride} placeholder="команда (пусто = из orchestrator.json)" />
        <label class="flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
          <span class="shrink-0">Автофикс</span>
          <select
            class="h-8 min-w-0 rounded-md border bg-background px-2 text-xs text-foreground"
            aria-label="Режим автофикса по алерту"
            value={settingsAutofix}
            onchange={(e) => {
              const v = parseAutofixMode((e.currentTarget as HTMLSelectElement).value);
              void saveAutofixMode(v);
            }}
          >
            {#each AUTOFIX_MODES as mode (mode)}
              <option value={mode}>{autofixModeLabel(mode)}</option>
            {/each}
          </select>
        </label>
      </div>
      <ul class="min-w-0 space-y-0.5 overflow-x-hidden">
        {@render tree(projects, 0)}
      </ul>
    </Card.Content>
  </Card.Root>
{/snippet}

{#snippet processesView()}
  <Card.Root>
    <Card.Header>
      <Card.Title class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Процессы</Card.Title>
    </Card.Header>
    <Card.Content>
      {#if processes.length === 0}
        <p class="text-sm text-muted-foreground">Нет запущенных процессов.</p>
      {:else}
        <Table.Root>
          <Table.Header>
            <Table.Row>
              <Table.Head>Проект</Table.Head>
              <Table.Head>Команда</Table.Head>
              <Table.Head>Статус</Table.Head>
              <Table.Head></Table.Head>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each processes as row (row.id)}
              <Table.Row data-state={row.id === selectedId ? "selected" : undefined}>
                <Table.Cell>
                  <Button variant="link" class="h-auto p-0" onclick={() => select(row.id)} aria-pressed={row.id === selectedId}>{row.project}</Button>
                </Table.Cell>
                <Table.Cell class="font-mono text-xs">{row.command}</Table.Cell>
                <Table.Cell>
                  <Badge variant={isRunning(row.status) ? "default" : "secondary"}>{statusLabel(row.status)}</Badge>
                </Table.Cell>
                <Table.Cell class="text-right">
                  {#if isRunning(row.status)}
                    <Button size="sm" variant="outline" onclick={(e) => { e.stopPropagation(); stop(row.id); }}>Стоп</Button>
                  {/if}
                </Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      {/if}
    </Card.Content>
  </Card.Root>
{/snippet}

{#snippet portsView()}
  <PortsPanel {ports} {portsError} {portsBusy} bind:portInterval onRefresh={refreshPorts} onStopManaged={stopManagedFromPorts} />
{/snippet}

{#snippet tasksView()}
  <TasksPanel
    {tasks}
    {taskErrors}
    {tasksBusy}
    bind:projectFilter
    bind:agentRunner
    {agentCatalog}
    projects={flatProjects}
    {startBusyTaskId}
    onRefresh={refreshTasks}
    onOpen={openTask}
    onStartAgent={startAgentFromTask}
  />
{/snippet}

{#snippet agentsView()}
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
{/snippet}

{#snippet logsView()}
  <Card.Root class="h-full">
    <Card.Header>
      <Card.Title class="text-xs font-medium uppercase tracking-wider text-muted-foreground">
        Логи {selectedId !== null ? `#${selectedId}` : ""}
      </Card.Title>
    </Card.Header>
    <Card.Content>
      <ScrollArea class="h-96 rounded-md border bg-background">
        <pre class="p-3 font-mono text-xs leading-relaxed text-muted-foreground">{#each logs as line, i (i)}<span class={line.stream === "stderr" ? "text-destructive" : ""}>{line.text}
</span>{/each}</pre>
      </ScrollArea>
    </Card.Content>
  </Card.Root>
{/snippet}

<main class="mx-auto min-h-screen max-w-[1400px] space-y-3 p-4">
  <header class="flex flex-wrap items-center gap-3">
    <h1 class="text-base font-medium tracking-widest text-primary">DEMERZEL</h1>
    {#if focusProject}
      <Badge variant="outline" role="status">Открыто из уведомления: {focusProject}</Badge>
    {/if}
    <ToggleGroup.Root
      type="single"
      variant="outline"
      class="ml-auto"
      aria-label="Вид окна"
      value={view}
      onValueChange={(v) => v && setView(v as ViewMode)}
    >
      <ToggleGroup.Item value="columns">Колонки</ToggleGroup.Item>
      <ToggleGroup.Item value="tabs">Вкладки</ToggleGroup.Item>
    </ToggleGroup.Root>
  </header>

  {#if error}
    <p class="break-words text-sm text-destructive">{error}</p>
  {/if}

  {#if autofixOffer}
    <div
      class="flex flex-wrap items-center gap-2 rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm"
      role="status"
      aria-live="polite"
    >
      <span class="min-w-0 flex-1 break-words">
        Автофикс: {autofixOffer.project} — {autofixOffer.message}
      </span>
      <Button size="sm" disabled={autofixBusy} onclick={() => void confirmAutofixOffer()}>
        {autofixBusy ? "Запуск…" : "Исправить"}
      </Button>
      <Button size="sm" variant="outline" disabled={autofixBusy} onclick={dismissAutofixOffer}>
        Отклонить
      </Button>
    </div>
  {/if}

  {#if view === "columns"}
    <Resizable.PaneGroup direction="horizontal" class="min-h-[560px] min-w-0 rounded-xl border">
      <Resizable.Pane defaultSize={24} minSize={16} class="min-w-0 overflow-hidden">
        <div class="h-full min-w-0 overflow-x-hidden overflow-y-auto p-2">{@render projectsView()}</div>
      </Resizable.Pane>
      <Resizable.Handle withHandle />
      <Resizable.Pane defaultSize={48} minSize={25} class="min-w-0 overflow-hidden">
        <div class="min-w-0 space-y-3 overflow-x-hidden p-2">
          {@render processesView()}
          {@render portsView()}
          {@render tasksView()}
          {@render agentsView()}
        </div>
      </Resizable.Pane>
      <Resizable.Handle withHandle />
      <Resizable.Pane defaultSize={28} minSize={16} class="min-w-0 overflow-hidden">
        <div class="h-full min-w-0 overflow-x-hidden overflow-y-auto p-2">{@render logsView()}</div>
      </Resizable.Pane>
    </Resizable.PaneGroup>
  {:else}
    <Tabs.Root value={tab} onValueChange={(v) => (tab = v as Tab)}>
      <Tabs.List>
        {#each TABS as [id, label] (id)}
          <Tabs.Trigger value={id}>{label}</Tabs.Trigger>
        {/each}
      </Tabs.List>
    </Tabs.Root>
    <div class="space-y-3">
      {#if tab === "projects"}{@render projectsView()}{/if}
      {#if tab === "processes"}{@render processesView()}{/if}
      {#if tab === "ports"}{@render portsView()}{/if}
      {#if tab === "tasks"}{@render tasksView()}{/if}
      {#if tab === "agents"}{@render agentsView()}{/if}
      {#if tab === "logs"}{@render logsView()}{/if}
    </div>
  {/if}
</main>
