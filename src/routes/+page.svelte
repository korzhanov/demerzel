<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import AutomationsPanel from "$lib/components/AutomationsPanel.svelte";
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
  import {
    enabledRootPaths,
    entriesFromLegacyRoots,
    type ScanRootEntry,
  } from "$lib/scan-roots";
  import { AUTOFIX_OFFER_EVENT, OPEN_PROJECT_EVENT, processIdForProject } from "$lib/alerts";
  import { clampPortPollInterval, portStatusLabel } from "$lib/ports";
  import {
    canOpenTask,
    canStartAgentFromTask,
    countByStatus,
    filterTasksByProject,
    formatTaskBoardSummary,
  } from "$lib/tasks";
  import {
    buildDashboardSummary,
    dotToneClass,
    flattenProjects,
    formatUpdatedAt,
    processForProject,
    projectRowStatus,
    statusToneClass,
  } from "$lib/dashboard";
  import PortsPanel from "$lib/components/PortsPanel.svelte";
  import TasksPanel from "$lib/components/TasksPanel.svelte";
  import AgentRunsPanel from "$lib/components/AgentRunsPanel.svelte";
  import StatusSummary from "$lib/components/StatusSummary.svelte";
  import ScanSettingsDialog from "$lib/components/ScanSettingsDialog.svelte";
  import AgentsSettingsDialog from "$lib/components/AgentsSettingsDialog.svelte";
  import LogsLayer from "$lib/components/LogsLayer.svelte";
  import {
    mergeCatalogPrefs,
    type AgentCatalogPref,
  } from "$lib/agent-install";
  import { AGENT_RUNNERS } from "$lib/types";
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import * as Tabs from "$lib/components/ui/tabs/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Resizable from "$lib/components/ui/resizable/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import FolderTreeIcon from "@lucide/svelte/icons/folder-tree";
  import ScrollTextIcon from "@lucide/svelte/icons/scroll-text";
  import Columns2Icon from "@lucide/svelte/icons/columns-2";
  import LayoutListIcon from "@lucide/svelte/icons/layout-list";
  import AppWindowIcon from "@lucide/svelte/icons/app-window";
  import SunIcon from "@lucide/svelte/icons/sun";
  import MoonIcon from "@lucide/svelte/icons/moon";
  import MonitorIcon from "@lucide/svelte/icons/monitor";
  import BotIcon from "@lucide/svelte/icons/bot";
  import {
    applyTheme,
    nextTheme,
    readStoredTheme,
    themeLabel,
    toggleTheme,
    type ThemeMode,
  } from "$lib/theme";

  let scanEntries = $state<ScanRootEntry[]>([]);
  let theme = $state<ThemeMode>("dark");
  let widgetBusy = $state(false);
  let widgetMsg = $state("");
  let agentsSettingsOpen = $state(false);
  let agentCatalogPrefs = $state<AgentCatalogPref[]>([]);
  let projects = $state<Project[]>([]);
  let processes = $state<ProcessRow[]>([]);
  let selectedId = $state<number | null>(null);
  /** Выбранный проект в сайдбаре (мокup columns). */
  let selectedProjectPath = $state<string | null>(null);
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
  /** Модалка корней сканирования + автофикс (редко нужна в узком сайдбаре). */
  let scanSettingsOpen = $state(false);
  let taskGeneration = 0;
  let updatedAt = $state(formatUpdatedAt(new Date()));
  const flatProjects = $derived(flattenProjects(projects));
  const rootsCount = $derived(enabledRootPaths(scanEntries).length);
  const summary = $derived(
    buildDashboardSummary({ projects, processes, ports, tasks }),
  );
  const selectedProject = $derived(
    selectedProjectPath
      ? (flatProjects.find((p) => p.path === selectedProjectPath) ?? null)
      : (flatProjects[0] ?? null),
  );
  const selectedProcess = $derived(
    selectedProject ? processForProject(processes, selectedProject.name) : undefined,
  );
  const selectedProjectPorts = $derived(
    selectedProject
      ? ports.filter(
          (row) =>
            row.project === selectedProject.name || row.project === selectedProject.path,
        )
      : [],
  );
  const selectedTasksLine = $derived(
    formatTaskBoardSummary(
      countByStatus(filterTasksByProject(tasks, selectedProject?.path ?? "")),
    ),
  );

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
      const result = await invoke<PortRow[]>("list_ports", {
        paths: flatProjects.map((p) => p.path),
      });
      if (mounted) {
        ports = result;
        portsError = "";
        updatedAt = formatUpdatedAt(new Date());
      }
    } catch (e) {
      if (mounted) portsError = String(e);
    } finally {
      portsBusy = false;
    }
  }

  async function refreshTasks() {
    const generation = ++taskGeneration;
    tasksBusy = true;
    const results = await Promise.all(
      flatProjects.map(async (p) => {
        try {
          return { tasks: await invoke<Task[]>("project_tasks", { path: p.path }), error: "" };
        } catch (e) {
          return { tasks: [] as Task[], error: `${p.name} (${p.path}): ${String(e)}` };
        }
      }),
    );
    if (!mounted || generation !== taskGeneration) return;
    tasks = results.flatMap((result) => result.tasks);
    taskErrors = results.map((result) => result.error).filter(Boolean);
    tasksBusy = false;
    updatedAt = formatUpdatedAt(new Date());
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
    return enabledRootPaths(scanEntries);
  }

  async function saveScanEntries(next?: ScanRootEntry[]) {
    const entries = next ?? scanEntries;
    if (next) scanEntries = next;
    try {
      const current = await invoke<Settings>("get_settings");
      await invoke("save_settings", {
        settings: {
          ...current,
          scan_entries: entries,
          scan_roots: enabledRootPaths(entries),
          agents: {
            ...(current.agents ?? {}),
            runner: settingsAgentRunner,
            autofix: settingsAutofix,
          },
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
          scan_entries: scanEntries,
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

  async function saveAgentCatalogPrefs(next: AgentCatalogPref[]) {
    agentCatalogPrefs = next;
    try {
      const current = await invoke<Settings>("get_settings");
      await invoke("save_settings", {
        settings: {
          ...current,
          agents: {
            ...(current.agents ?? {}),
            runner: settingsAgentRunner,
            autofix: settingsAutofix,
            catalog: next,
          },
        } as Settings,
      });
      const preferred = await invoke<string>("resolve_preferred_agent");
      agentRunner = parseAgentRunner(preferred);
      settingsAgentRunner = agentRunner;
    } catch (e) {
      error = String(e);
    }
  }

  async function installWidgets() {
    widgetBusy = true;
    widgetMsg = "";
    error = "";
    try {
      widgetMsg = await invoke<string>("install_widgets");
    } catch (e) {
      error = String(e);
    } finally {
      widgetBusy = false;
    }
  }

  function onToggleTheme() {
    theme = toggleTheme(theme);
  }

  function onCycleView() {
    setView(view === "columns" ? "tabs" : "columns");
  }

  async function scan() {
    error = "";
    if (scanBusy) return;
    scanBusy = true;
    await saveScanEntries();
    try {
      const found = await invoke<Project[]>("scan_projects", { roots: rootList() });
      if (!mounted) return;
      projects = found;
      const flat = flattenProjects(found);
      if (!flat.some((p) => p.path === projectFilter)) projectFilter = "";
      if (!flat.some((p) => p.path === selectedProjectPath)) {
        selectedProjectPath = flat[0]?.path ?? null;
      }
      await refreshTasks();
    } catch (e) {
      error = String(e);
    } finally {
      scanBusy = false;
    }
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
      updatedAt = formatUpdatedAt(new Date());
    } catch (e) {
      error = String(e);
    } finally {
      processBusy = false;
    }
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
      selectedProjectPath = p.path;
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

  function selectProject(p: Project) {
    selectedProjectPath = p.path;
    projectFilter = p.path;
    const id = processIdForProject(processes, p.name);
    if (id !== null) select(id);
  }

  async function openInEditor(p: Project) {
    try {
      await openPath(p.path);
    } catch (e) {
      error = String(e);
    }
  }

  function openProjectFromNotify(project: string) {
    const id = processIdForProject(processes, project);
    if (id !== null) select(id);
    const match = flatProjects.find((p) => p.name === project);
    if (match) {
      selectedProjectPath = match.path;
      projectFilter = match.path;
    }
  }

  onMount(() => {
    mounted = true;
    theme = readStoredTheme();
    applyTheme(theme);
    const mq =
      typeof matchMedia !== "undefined"
        ? matchMedia("(prefers-color-scheme: dark)")
        : null;
    const onScheme = () => {
      if (readStoredTheme() === "system") {
        applyTheme("system");
        theme = "system";
      }
    };
    mq?.addEventListener("change", onScheme);
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
        scanEntries =
          s.scan_entries && s.scan_entries.length > 0
            ? s.scan_entries
            : entriesFromLegacyRoots(s.scan_roots ?? []);
        settingsAgentRunner = parseAgentRunner(s.agents?.runner);
        agentRunner = settingsAgentRunner;
        settingsAutofix = parseAutofixMode(s.agents?.autofix);
        agentCatalogPrefs = mergeCatalogPrefs(
          (s.agents?.catalog ?? []).map((p) => ({
            runner: parseAgentRunner(p.runner),
            enabled: p.enabled !== false,
          })),
          AGENT_RUNNERS,
        );
        if (rootList().length) await scan();
      } catch (e) {
        if (mounted) error = String(e);
      }
      try {
        const catalog = await invoke<AgentCatalogEntry[]>("list_available_agents");
        if (mounted) agentCatalog = catalog;
        try {
          const preferred = await invoke<string>("resolve_preferred_agent");
          if (mounted) {
            agentRunner = parseAgentRunner(preferred);
            settingsAgentRunner = agentRunner;
          }
        } catch {
          /* optional */
        }
      } catch (e) {
        if (mounted) taskErrors = [...taskErrors, String(e)];
      }
    })();
    void refresh();
    void refreshPorts();
    void refreshAgentRuns();
    const poll = setInterval(() => {
      void refresh();
    }, 1000);
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
      mq?.removeEventListener("change", onScheme);
      unlistenOpen?.();
      unlistenAutofix?.();
    };
  });

  type ViewMode = "columns" | "tabs";
  type Tab = "projects" | "processes" | "tasks" | "agents";
  const TABS: [Tab, string][] = [
    ["projects", "Проекты и порты"],
    ["processes", "Процессы"],
    ["tasks", "Задачи"],
    ["agents", "Агенты"],
  ];
  function readPref(key: string): string | null {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  }
  function writePref(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      /* без хранилища режим не запоминается */
    }
  }
  let view = $state<ViewMode>(readPref("demerzel.view") === "tabs" ? "tabs" : "columns");
  let tab = $state<Tab>("projects");
  let logsOpen = $state(readPref("demerzel.logsOpen") !== "0");
  let logsWidth = $state(Number(readPref("demerzel.logsWidth")) || 320);
  function setView(v: ViewMode) {
    view = v;
    writePref("demerzel.view", v);
  }
  $effect(() => {
    writePref("demerzel.logsOpen", logsOpen ? "1" : "0");
  });
  $effect(() => {
    writePref("demerzel.logsWidth", String(Math.round(logsWidth)));
  });
</script>

{#snippet tree(list: Project[], depth: number)}
  {#each list as p (p.path)}
    {@const row = projectRowStatus(p, processes)}
    <li>
      <button
        type="button"
        style="padding-left: {12 + depth * 14}px"
        class={[
          "flex w-full min-w-0 items-center gap-2 border-l-2 border-transparent py-1.5 pr-3 text-left text-sm transition-colors hover:bg-accent",
          selectedProjectPath === p.path && "border-l-primary bg-secondary",
        ]}
        title="{p.name} — {p.path}"
        onclick={() => selectProject(p)}
      >
        <span class={["inline-block size-2 shrink-0 rounded-full", dotToneClass(row.tone)]} aria-hidden="true"></span>
        <span class="min-w-0 flex-1 truncate">{p.name}</span>
        <span class={["shrink-0 text-xs", statusToneClass(row.tone)]}>{row.label}</span>
      </button>
      {#if p.children.length}
        <ul class="min-w-0">
          {@render tree(p.children, depth + 1)}
        </ul>
      {/if}
    </li>
  {/each}
{/snippet}

{#snippet projectsView()}
  <section class="flex min-w-0 flex-col" aria-labelledby="projects-title">
    <div class="flex items-center gap-2 px-3 pb-1.5 pt-3">
      <h2
        id="projects-title"
        class="min-w-0 flex-1 text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground"
      >
        Проекты
      </h2>
      <Button
        size="sm"
        variant="outline"
        class="h-7 shrink-0 gap-1 px-2 text-xs"
        onclick={() => (scanSettingsOpen = true)}
        aria-haspopup="dialog"
        aria-label={rootsCount > 0 ? `Корни сканирования · ${rootsCount}` : "Корни сканирования"}
        title="Корневые папки и автофикс"
      >
        <FolderTreeIcon class="size-3.5" />
        {#if rootsCount > 0}
          <span class="tabular-nums">{rootsCount}</span>
        {/if}
      </Button>
    </div>
    <ul class="min-w-0 overflow-x-hidden border-t border-border pb-2">
      {@render tree(projects, 0)}
    </ul>
    <div class="min-w-0 border-t border-border px-3 py-3">
      {@render portsView()}
    </div>
  </section>
{/snippet}

{#snippet processesView()}
  <Card.Root class="gap-3 rounded-lg py-3 shadow-none">
    <Card.Header class="px-4 pb-0">
      <Card.Title class="text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground"
        >Процессы</Card.Title
      >
    </Card.Header>
    <Card.Content class="px-4">
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
                  <Button
                    variant="link"
                    class="h-auto p-0"
                    onclick={() => select(row.id)}
                    aria-pressed={row.id === selectedId}>{row.project}</Button
                  >
                </Table.Cell>
                <Table.Cell class="font-mono text-xs">{row.command}</Table.Cell>
                <Table.Cell>
                  <Badge variant={isRunning(row.status) ? "default" : "secondary"}
                    >{statusLabel(row.status)}</Badge
                  >
                </Table.Cell>
                <Table.Cell class="text-right">
                  {#if isRunning(row.status)}
                    <Button
                      size="sm"
                      variant="outline"
                      onclick={(e) => {
                        e.stopPropagation();
                        stop(row.id);
                      }}>Стоп</Button
                    >
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
  <PortsPanel
    class="border-0 bg-transparent p-0 shadow-none"
    {ports}
    {portsError}
    {portsBusy}
    bind:portInterval
    onRefresh={refreshPorts}
    onStopManaged={stopManagedFromPorts}
  />
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

{#snippet projectFocus()}
  {#if selectedProject}
    {@const proc = selectedProcess}
    <div class="space-y-3 border-b border-border px-5 py-4">
      <div>
        <h2 class="text-lg font-medium tracking-tight">{selectedProject.name}</h2>
        <p class="truncate text-sm text-muted-foreground" title={selectedProject.path}>
          app · {selectedProject.path}
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-1.5">
        <Button size="sm" onclick={() => start(selectedProject)}>Запустить</Button>
        <Button
          size="sm"
          variant="outline"
          disabled={!proc || !isRunning(proc.status)}
          onclick={() => proc && stop(proc.id)}>Стоп</Button
        >
        <Button size="sm" variant="outline" onclick={() => openInEditor(selectedProject)}
          >Редактор</Button
        >
      </div>
      <Input
        class="w-full min-w-0 text-xs"
        bind:value={commandOverride}
        placeholder="команда (пусто = orchestrator.json)"
        aria-label="Команда запуска"
      />

      <AutomationsPanel path={selectedProject.path} name={selectedProject.name} />

      <div>
        <p class="mb-1 text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">
          Процессы
        </p>
        {#if proc}
          <div
            class="grid grid-cols-[1fr_90px_70px] gap-2 border-b border-border py-1.5 text-sm"
          >
            <span class="truncate font-mono text-xs">{proc.command}</span>
            <span class={isRunning(proc.status) ? "text-[var(--status-ok)]" : "text-muted-foreground"}
              >{statusLabel(proc.status)}</span
            >
            <span class="text-[var(--status-ok)]"
              >{proc.port != null ? `:${proc.port}` : "—"}</span
            >
          </div>
        {:else}
          <p class="text-sm text-muted-foreground">Нет процесса</p>
        {/if}
      </div>

      <div>
        <p class="mb-1 text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">
          Порты
        </p>
        {#if selectedProjectPorts.length === 0}
          <p class="text-sm text-muted-foreground">Нет связанных портов</p>
        {:else}
          {#each selectedProjectPorts as row (`${row.pid}:${row.port}`)}
            <div
              class="grid grid-cols-[1fr_90px_70px] gap-2 border-b border-border py-1.5 text-sm"
            >
              <span>{row.port}</span>
              <span
                class={row.status === "conflict"
                  ? "text-primary"
                  : row.status === "ok"
                    ? "text-[var(--status-ok)]"
                    : "text-muted-foreground"}>{portStatusLabel(row.status)}</span
              >
              <span class="truncate font-mono text-xs text-muted-foreground">{row.command}</span>
            </div>
          {/each}
        {/if}
      </div>

      <div>
        <p class="mb-1 text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">
          Задачи
        </p>
        <p class="text-sm text-muted-foreground">{selectedTasksLine}</p>
      </div>
    </div>
  {:else}
    <div class="border-b border-border px-5 py-6 text-sm text-muted-foreground">
      Выберите проект в списке слева или выполните сканирование.
    </div>
  {/if}
{/snippet}

<main class="relative mx-auto flex min-h-screen max-w-[1400px] flex-col">
  <ScanSettingsDialog
    bind:open={scanSettingsOpen}
    bind:entries={scanEntries}
    bind:autofix={settingsAutofix}
    {scanBusy}
    onScan={scan}
    onAutofixChange={saveAutofixMode}
    onPersist={saveScanEntries}
  />
  <AgentsSettingsDialog
    bind:open={agentsSettingsOpen}
    bind:catalog={agentCatalog}
    bind:prefs={agentCatalogPrefs}
    onPersist={saveAgentCatalogPrefs}
  />

  <header class="shrink-0 border-b border-border">
    <h1 class="sr-only">DEMERZEL</h1>
    <div
      class="h-[var(--banner-height)] bg-[url('/mockups/strip.jpg')] bg-cover bg-center"
      role="img"
      aria-label="DEMERZEL — The Shadow Orchestrator for Your Stack"
    ></div>
  </header>

  {#if widgetMsg}
    <p class="px-4 pt-2 text-sm text-[var(--status-ok)]" role="status">{widgetMsg}</p>
  {/if}

  <Tooltip.Provider delayDuration={300}>
    <StatusSummary {summary} {updatedAt}>
      {#snippet actions()}
        <div
          class="flex flex-wrap items-center gap-1"
          role="toolbar"
          aria-label="Панель окна"
        >
          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  size="sm"
                  variant="outline"
                  class="size-8 p-0"
                  aria-label="Агенты"
                  onclick={() => (agentsSettingsOpen = true)}
                >
                  <BotIcon class="size-4" />
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content side="bottom" align="end"
              >Агенты: приоритет и установка</Tooltip.Content
            >
          </Tooltip.Root>

          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  size="sm"
                  variant="outline"
                  class="size-8 p-0"
                  disabled={widgetBusy}
                  aria-label="Установить виджеты"
                  onclick={() => void installWidgets()}
                >
                  <AppWindowIcon class="size-4" />
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content side="bottom" align="end"
              >Установить виджеты в /Applications (нужен tauri build --debug)</Tooltip.Content
            >
          </Tooltip.Root>

          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  size="sm"
                  variant={logsOpen ? "default" : "outline"}
                  class="size-8 p-0"
                  aria-pressed={logsOpen}
                  aria-label={logsOpen ? "Скрыть логи" : "Показать логи"}
                  onclick={() => (logsOpen = !logsOpen)}
                >
                  <ScrollTextIcon class="size-4" />
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content>{logsOpen ? "Скрыть логи" : "Показать логи"}</Tooltip.Content>
          </Tooltip.Root>

          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  size="sm"
                  variant="outline"
                  class="size-8 p-0"
                  aria-label={view === "columns" ? "Вид: колонки" : "Вид: вкладки"}
                  onclick={onCycleView}
                >
                  {#if view === "columns"}
                    <Columns2Icon class="size-4" />
                  {:else}
                    <LayoutListIcon class="size-4" />
                  {/if}
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content
              >{view === "columns"
                ? "Колонки · далее вкладки"
                : "Вкладки · далее колонки"}</Tooltip.Content
            >
          </Tooltip.Root>

          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  size="sm"
                  variant="outline"
                  class="size-8 p-0"
                  aria-label={themeLabel(theme)}
                  onclick={onToggleTheme}
                >
                  {#if theme === "dark"}
                    <MoonIcon class="size-4" />
                  {:else if theme === "light"}
                    <SunIcon class="size-4" />
                  {:else}
                    <MonitorIcon class="size-4" />
                  {/if}
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content
              >{themeLabel(theme)} · далее {themeLabel(nextTheme(theme))}</Tooltip.Content
            >
          </Tooltip.Root>
        </div>
      {/snippet}
    </StatusSummary>
  </Tooltip.Provider>

  {#if error}
    <p class="break-words px-4 pt-2 text-sm text-destructive">{error}</p>
  {/if}

  {#if autofixOffer}
    <div
      class="mx-4 mt-2 flex flex-wrap items-center gap-2 rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm"
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

  <div class="relative min-h-0 min-w-0 flex-1">
    {#if view === "columns"}
      <Resizable.PaneGroup
        direction="horizontal"
        class="min-h-[560px] min-w-0 flex-1 border-t border-[var(--window-edge)]/40"
      >
        <Resizable.Pane defaultSize={28} minSize={14} class="min-w-0 overflow-hidden bg-card/40">
          <div class="h-full min-w-0 overflow-x-hidden overflow-y-auto border-r border-border">
            {@render projectsView()}
          </div>
        </Resizable.Pane>
        <Resizable.Handle withHandle />
        <Resizable.Pane defaultSize={72} minSize={36} class="min-w-0 overflow-hidden">
          <div class="h-full min-w-0 space-y-3 overflow-x-hidden overflow-y-auto pb-4">
            {@render projectFocus()}
            <div class="space-y-3 px-3">
              {@render tasksView()}
              {@render agentsView()}
            </div>
          </div>
        </Resizable.Pane>
      </Resizable.PaneGroup>
    {:else}
      <div class="space-y-3 p-4">
        <Tabs.Root value={tab} onValueChange={(v) => (tab = v as Tab)}>
          <Tabs.List class="h-auto flex-wrap gap-1 bg-transparent p-0">
            {#each TABS as [id, label] (id)}
              <Tabs.Trigger
                value={id}
                class="rounded-md border border-[var(--window-edge)] data-[state=active]:border-primary data-[state=active]:bg-primary data-[state=active]:text-primary-foreground"
                >{label}</Tabs.Trigger
              >
            {/each}
          </Tabs.List>
        </Tabs.Root>
        <div class="space-y-3">
          {#if tab === "projects"}{@render projectsView()}{/if}
          {#if tab === "processes"}{@render processesView()}{/if}
          {#if tab === "tasks"}{@render tasksView()}{/if}
          {#if tab === "agents"}{@render agentsView()}{/if}
        </div>
      </div>
    {/if}

    <LogsLayer
      bind:open={logsOpen}
      bind:width={logsWidth}
      {logs}
      projectName={selectedProject?.name ?? null}
    />
  </div>
</main>
