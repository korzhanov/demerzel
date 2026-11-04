<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { emit } from "@tauri-apps/api/event";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { openPath, openUrl } from "@tauri-apps/plugin-opener";
  import type { Component } from "svelte";
  import type { AgentCatalogEntry, AutofixMode, Project, Settings, SystemJob } from "$lib/types";
  import { AGENT_RUNNERS, parseAgentRunner, parseAutofixMode } from "$lib/types";
  import { enabledRootPaths, entriesFromLegacyRoots, type ScanRootEntry } from "$lib/scan-roots";
  import { mergeCatalogPrefs, type AgentCatalogPref } from "$lib/agent-install";
  import { applyTheme, readStoredTheme, themeLabel, type ThemeMode } from "$lib/theme";
  import { buildLabel } from "$lib/build-info";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import ScanSettingsDialog from "$lib/components/ScanSettingsDialog.svelte";
  import AgentsSettingsDialog from "$lib/components/AgentsSettingsDialog.svelte";
  import UpdateControl from "$lib/components/UpdateControl.svelte";
  import LoadingSplash from "$lib/components/LoadingSplash.svelte";
  import SearchIcon from "@lucide/svelte/icons/search";
  import XIcon from "@lucide/svelte/icons/x";
  import SlidersHorizontalIcon from "@lucide/svelte/icons/sliders-horizontal";
  import FolderTreeIcon from "@lucide/svelte/icons/folder-tree";
  import BotIcon from "@lucide/svelte/icons/bot";
  import BellIcon from "@lucide/svelte/icons/bell";
  import AppWindowIcon from "@lucide/svelte/icons/app-window";
  import CalendarDaysIcon from "@lucide/svelte/icons/calendar-days";
  import InfoIcon from "@lucide/svelte/icons/info";

  let { open = $bindable(false), windowed = false }: { open: boolean; windowed?: boolean } = $props();

  type Section = "general" | "projects" | "agents" | "notifications" | "widget" | "planning" | "calendar" | "about";
  const sections: { id: Section; label: string; description: string; Icon: Component }[] = [
    { id: "general", label: "Основные", description: "Тема оформления импорт экспорт файла", Icon: SlidersHorizontalIcon },
    { id: "projects", label: "Проекты", description: "Директории папки корни сканирование", Icon: FolderTreeIcon },
    { id: "agents", label: "AI агенты", description: "Приоритет установка запуск", Icon: BotIcon },
    { id: "notifications", label: "Уведомления", description: "Алерты тихие часы звук", Icon: BellIcon },
    { id: "widget", label: "Виджет", description: "WidgetKit установить синхронизация", Icon: AppWindowIcon },
    { id: "planning", label: "Планирование", description: "Расписания crontab launchd", Icon: CalendarDaysIcon },
    { id: "calendar", label: "Календарь", description: "Экспорт активности процессов исправлений токены ics", Icon: CalendarDaysIcon },
    { id: "about", label: "О проекте", description: "Demerzel версия обновление GitHub", Icon: InfoIcon },
  ];

  let section = $state<Section>("general");
  let search = $state("");
  let settingsReady = $state(false);
  let settingsLoadStarted = false;
  let message = $state("");
  let error = $state("");
  let busy = $state(false);
  let theme = $state<ThemeMode>("dark");
  let loaded = $state<Settings | null>(null);
  let scanEntries = $state<ScanRootEntry[]>([]);
  let autofix = $state<AutofixMode>("off");
  let projectScanBusy = $state(false);
  let projectCount = $state<number | null>(null);
  let agentCatalog = $state<AgentCatalogEntry[]>([]);
  let agentPrefs = $state<AgentCatalogPref[]>([]);
  let agentsLoaded = $state(false);
  let notificationBusy = $state(false);
  let notificationMessage = $state("");
  let widgetBusy = $state(false);
  let widgetMessage = $state("");
  let dedupe = $state(600);
  let voiceGap = $state(60);
  let quietOn = $state(false);
  let quietStart = $state("23:00");
  let quietEnd = $state("08:00");
  let timezone = $state("Europe/Berlin");
  let sysOn = $state(false);
  let sysJobs = $state<SystemJob[]>([]);
  let sysError = $state("");
  let planningLoaded = $state(false);
  let calendarBusy = $state(false);
  let calendarMessage = $state("");
  let alertsLoaded = $state(false);

  const visibleSections = $derived(
    sections.filter((item) => `${item.label} ${item.description}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase())),
  );
  const HHMM = /^([01]\d|2[0-3]):[0-5]\d$/;
  const themeModes: ThemeMode[] = ["system", "light", "dark"];

  function close() {
    open = false;
    message = "";
    error = "";
  }

  function applySettings(settings: Settings) {
    loaded = settings;
    scanEntries = settings.scan_entries?.length
      ? settings.scan_entries
      : entriesFromLegacyRoots(settings.scan_roots ?? []);
    autofix = parseAutofixMode(settings.agents?.autofix);
    agentPrefs = mergeCatalogPrefs(
      (settings.agents?.catalog ?? []).map((pref) => ({
        runner: parseAgentRunner(pref.runner),
        enabled: pref.enabled !== false,
      })),
      AGENT_RUNNERS,
    );
    theme = readStoredTheme();
    applyTheme(theme);
  }

  async function loadSettings() {
    try {
      applySettings(await invoke<Settings>("get_settings"));
    } catch (e) {
      error = String(e);
    } finally {
      settingsReady = true;
    }
  }

  async function saveSettings(next: Settings) {
    await invoke("save_settings", { settings: next });
    applySettings(next);
  }

  async function exportSettings() {
    error = "";
    message = "";
    try {
      const path = await saveDialog({
        defaultPath: "demerzel-settings.json",
        filters: [{ name: "Настройки", extensions: ["json", "yaml", "yml"] }],
      });
      if (!path) return;
      busy = true;
      await invoke("export_settings", { path });
      message = `Сохранено: ${path}`;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function importSettings() {
    error = "";
    message = "";
    try {
      const path = await openDialog({ multiple: false, filters: [{ name: "Настройки", extensions: ["json", "yaml", "yml"] }] });
      if (!path || typeof path !== "string") return;
      busy = true;
      const incoming = await invoke<Settings>("read_settings_file", { path });
      await saveSettings(incoming);
      alertsLoaded = false;
      planningLoaded = false;
      message = "Настройки импортированы.";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function selectTheme(mode: ThemeMode) {
    theme = mode;
    applyTheme(theme);
    try {
      await emit("demerzel-theme-changed", { theme });
    } catch {
      // В браузерном предпросмотре событие Tauri недоступно; локальная тема уже обновлена.
    }
  }

  async function openRepository() {
    try {
      await openUrl("https://github.com/korzhanov/demerzel");
    } catch (e) {
      error = String(e);
    }
  }

  async function chooseCalendarFile() {
    if (!loaded) return;
    calendarMessage = "";
    try {
      const path = await saveDialog({
        defaultPath: loaded.calendar?.export_path ?? "demerzel-activity.ics",
        filters: [{ name: "Календарь iCalendar", extensions: ["ics"] }],
      });
      if (!path) return;
      await saveSettings({
        ...loaded,
        calendar: {
          export_path: path,
          include_processes: loaded.calendar?.include_processes ?? true,
          include_agents: loaded.calendar?.include_agents ?? true,
        },
      });
      await invoke("export_calendar");
      calendarMessage = "Файл .ics создан и будет обновляться после завершения процессов и исправлений. Чтобы увидеть новые события в календаре, импортируйте обновлённый файл.";
    } catch (e) {
      error = String(e);
    }
  }

  async function exportCalendarNow() {
    calendarBusy = true;
    calendarMessage = "";
    try {
      await invoke("export_calendar");
      calendarMessage = "Файл календаря обновлён.";
    } catch (e) {
      error = String(e);
    } finally {
      calendarBusy = false;
    }
  }

  async function openCalendarFile() {
    const path = loaded?.calendar?.export_path;
    if (!path) return;
    try { await openPath(path); }
    catch (e) { error = String(e); }
  }

  async function persistProjectEntries(entries: ScanRootEntry[]) {
    try {
      const current = loaded ?? await invoke<Settings>("get_settings");
      await saveSettings({
        ...current,
        scan_entries: entries,
        scan_roots: enabledRootPaths(entries),
      });
    } catch (e) {
      error = String(e);
    }
  }

  async function saveAutofix(mode: AutofixMode) {
    autofix = mode;
    try {
      const current = loaded ?? await invoke<Settings>("get_settings");
      await saveSettings({
        ...current,
        agents: { ...(current.agents ?? {}), autofix: mode },
      });
    } catch (e) {
      error = String(e);
    }
  }

  async function scanProjects() {
    if (projectScanBusy) return;
    projectScanBusy = true;
    error = "";
    try {
      const found = await invoke<Project[]>("scan_projects", { roots: enabledRootPaths(scanEntries) });
      projectCount = found.length;
      await emit("demerzel-projects-scanned", { projects: found });
      message = `Найдено проектов: ${found.length}.`;
    } catch (e) {
      error = String(e);
    } finally {
      projectScanBusy = false;
    }
  }

  async function loadAgents() {
    if (agentsLoaded) return;
    try {
      if (!loaded) await loadSettings();
      agentCatalog = await invoke<AgentCatalogEntry[]>("list_available_agents");
    } catch (e) {
      error = String(e);
    } finally {
      agentsLoaded = true;
    }
  }

  async function saveAgentPrefs(next: AgentCatalogPref[]) {
    try {
      const current = loaded ?? await invoke<Settings>("get_settings");
      await saveSettings({
        ...current,
        agents: { ...(current.agents ?? {}), catalog: next },
      });
    } catch (e) {
      error = String(e);
    }
  }

  async function testNotifications() {
    notificationBusy = true;
    notificationMessage = "";
    try {
      const result = await invoke<{ spoken: boolean; speak_error: string | null }>("test_notification");
      notificationMessage = result.spoken
        ? "Уведомление и голос запущены."
        : `Уведомление запущено · голос: ${result.speak_error ?? "не запущен"}`;
    } catch (e) {
      notificationMessage = `Ошибка уведомления: ${String(e)}`;
    } finally {
      notificationBusy = false;
    }
  }

  async function installWidgets() {
    widgetBusy = true;
    widgetMessage = "";
    try {
      widgetMessage = await invoke<string>("install_widgets");
    } catch (e) {
      widgetMessage = String(e);
    } finally {
      widgetBusy = false;
    }
  }

  async function saveAlerts() {
    error = "";
    message = "";
    if (!loaded) return;
    if (!Number.isInteger(dedupe) || dedupe < 0 || !Number.isInteger(voiceGap) || voiceGap < 0) {
      error = "Интервалы должны быть целыми секундами, не меньше нуля.";
      return;
    }
    if (quietOn && (!HHMM.test(quietStart) || !HHMM.test(quietEnd))) {
      error = "Тихие часы укажите в формате ЧЧ:ММ, например 23:00.";
      return;
    }
    busy = true;
    try {
      await saveSettings({
        ...loaded,
        alerts: {
          ...(loaded.alerts ?? { rules: [] }),
          dedupe_window_secs: dedupe,
          voice_min_interval_secs: voiceGap,
          quiet_hours: { enabled: quietOn, start: quietStart, end: quietEnd },
          timezone: timezone.trim() || "Europe/Berlin",
        },
      });
      message = "Сохранено.";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function loadAlerts() {
    if (!loaded) return;
    const alerts = loaded.alerts;
    dedupe = alerts?.dedupe_window_secs ?? 600;
    voiceGap = alerts?.voice_min_interval_secs ?? 60;
    quietOn = alerts?.quiet_hours?.enabled ?? false;
    quietStart = alerts?.quiet_hours?.start ?? "23:00";
    quietEnd = alerts?.quiet_hours?.end ?? "08:00";
    timezone = alerts?.timezone ?? "Europe/Berlin";
    alertsLoaded = true;
  }

  async function toggleSystemSchedules(on: boolean) {
    sysError = "";
    try {
      const current = loaded ?? await invoke<Settings>("get_settings");
      await saveSettings({ ...current, system_schedules: { enabled: on } });
      sysOn = on;
      if (on) await refreshSystemSchedules();
    } catch (e) {
      sysError = String(e);
    }
  }

  async function refreshSystemSchedules() {
    sysError = "";
    try {
      sysJobs = await invoke<SystemJob[]>("system_schedules");
    } catch (e) {
      sysError = String(e);
    }
  }

  async function loadPlanning() {
    if (!loaded) return;
    sysOn = loaded.system_schedules?.enabled ?? false;
    planningLoaded = true;
    if (sysOn) await refreshSystemSchedules();
  }

  $effect(() => {
    if (!open) return;
    if (!settingsLoadStarted) {
      settingsLoadStarted = true;
      void loadSettings();
    }
    if (section === "agents" && settingsReady) void loadAgents();
    if (section === "notifications" && settingsReady && !alertsLoaded) void loadAlerts();
    if (section === "planning" && settingsReady && !planningLoaded) void loadPlanning();
  });
</script>

{#if open}
  <div
    class={windowed ? "flex min-h-screen flex-col" : "fixed inset-0 z-40 flex items-center justify-center bg-black/45 p-3"}
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) close(); }}
  >
    <div
      class={windowed
        ? "relative flex min-h-screen w-full flex-col bg-card text-card-foreground"
        : "relative flex max-h-[min(90vh,46rem)] w-[min(60rem,calc(100vw-1.5rem))] flex-col overflow-hidden rounded-xl border border-border bg-card text-card-foreground shadow-lg"}
      role="dialog"
      aria-modal="true"
      aria-labelledby="app-settings-title"
    >
      <div class="flex min-h-0 flex-1">
        <aside class="flex w-52 shrink-0 flex-col border-r border-border bg-background/60 p-3">
          <div class="flex items-center justify-between px-2 pb-3 pt-1">
            <h2 id="app-settings-title" class="text-base font-semibold">Настройки</h2>
          </div>
          <label class="relative mb-3 block">
            <SearchIcon class="absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input class="h-8 pl-8 text-xs" type="search" placeholder="Поиск" bind:value={search} aria-label="Поиск по настройкам" />
          </label>
          <nav class="space-y-1" aria-label="Разделы настроек">
            {#each visibleSections as item (item.id)}
              <button
                type="button"
                aria-current={section === item.id ? "page" : undefined}
                class="flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors {section === item.id ? 'bg-secondary text-foreground' : 'text-muted-foreground hover:bg-accent hover:text-foreground'}"
                onclick={() => { section = item.id; message = ""; error = ""; }}
              >
                <item.Icon class="size-4 shrink-0" />
                <span class="truncate">{item.label}</span>
              </button>
            {/each}
            {#if visibleSections.length === 0}<p class="px-2.5 py-2 text-xs text-muted-foreground">Ничего не найдено</p>{/if}
          </nav>
          {#if !windowed}
            <Button size="sm" variant="ghost" class="mt-auto self-start" aria-label="Закрыть" onclick={close}>
              <XIcon class="mr-2 size-4" />Закрыть
            </Button>
          {/if}
        </aside>

        <section class="min-w-0 flex-1 overflow-y-auto">
          <div class="min-h-full p-6 text-sm">
            {#if !settingsReady}
              <div class="flex min-h-80 items-center justify-center text-muted-foreground" role="status">
                Загружаем настройки…
              </div>
            {:else if section === "general"}
              <header class="mb-6 border-b border-border pb-4">
                <h3 class="text-lg font-semibold">Основные</h3>
                <p class="mt-1 text-sm text-muted-foreground">Тема и резервная копия настроек.</p>
              </header>
              <section class="space-y-3">
                <div>
                  <p class="font-medium">Оформление</p>
                  <p class="mt-1 text-xs text-muted-foreground">Тёмная, светлая или системная тема.</p>
                </div>
                <div class="flex flex-wrap gap-2" role="group" aria-label="Выбор темы">
                  {#each themeModes as mode (mode)}
                    <Button size="sm" variant={theme === mode ? "default" : "outline"} aria-pressed={theme === mode} onclick={() => void selectTheme(mode)}>
                      {themeLabel(mode)}
                    </Button>
                  {/each}
                </div>
              </section>
              <section class="mt-6 space-y-2 border-t border-border pt-4">
                <p class="font-medium">Файл настроек</p>
                <p class="text-muted-foreground">Экспорт и импорт в JSON или YAML. Импорт заменяет текущие настройки.</p>
                <div class="flex flex-wrap gap-2">
                  <Button size="sm" variant="outline" disabled={busy} onclick={exportSettings}>Экспорт…</Button>
                  <Button size="sm" variant="outline" disabled={busy} onclick={importSettings}>Импорт…</Button>
                </div>
              </section>
            {:else if section === "projects"}
              <header class="mb-4 border-b border-border pb-4">
                <h3 class="text-lg font-semibold">Проекты</h3>
                <p class="mt-1 text-sm text-muted-foreground">Выберите директории, в которых Demerzel ищет проекты.</p>
              </header>
              <ScanSettingsDialog
                open={true}
                inline
                bind:entries={scanEntries}
                bind:autofix
                scanBusy={projectScanBusy}
                onScan={scanProjects}
                onAutofixChange={saveAutofix}
                onPersist={persistProjectEntries}
              />
              {#if projectCount !== null}<p class="mt-2 text-xs text-muted-foreground">Последнее сканирование: {projectCount} проектов.</p>{/if}
            {:else if section === "agents"}
              <header class="mb-4 border-b border-border pb-4">
                <h3 class="text-lg font-semibold">AI агенты</h3>
                <p class="mt-1 text-sm text-muted-foreground">Настройте порядок, доступность и установку агентов.</p>
              </header>
              {#if agentsLoaded}
                <AgentsSettingsDialog
                  inline
                  open={section === "agents"}
                  bind:catalog={agentCatalog}
                  bind:prefs={agentPrefs}
                  onPersist={saveAgentPrefs}
                />
              {:else}
                <p class="py-8 text-center text-muted-foreground">Загружаем список установленных агентов…</p>
              {/if}
            {:else if section === "notifications"}
              <header class="mb-6 border-b border-border pb-4">
                <h3 class="text-lg font-semibold">Уведомления</h3>
                <p class="mt-1 text-sm text-muted-foreground">Проверка уведомлений, голос, тихие часы и интервалы.</p>
              </header>
              <Button size="sm" variant="outline" disabled={notificationBusy} onclick={() => void testNotifications()}>
                {notificationBusy ? "Проверяем…" : "Проверить уведомление и голос"}
              </Button>
              {#if notificationMessage}<p class="mt-2 text-xs text-muted-foreground" role="status">{notificationMessage}</p>{/if}
              <section class="mt-6 space-y-3 border-t border-border pt-4">
                <p class="font-medium">Параметры оповещений</p>
                <label class="flex items-center justify-between gap-3"><span>Не повторять событие, сек</span><Input class="w-28" type="number" min="0" bind:value={dedupe} /></label>
                <label class="flex items-center justify-between gap-3"><span>Пауза между голосовыми оповещениями, сек</span><Input class="w-28" type="number" min="0" bind:value={voiceGap} /></label>
                <label class="flex items-center justify-between gap-3"><span>Часовой пояс</span><Input class="w-44" bind:value={timezone} /></label>
                <div class="flex items-center gap-2"><Checkbox id="quiet-hours" checked={quietOn} onCheckedChange={(v) => (quietOn = v === true)} /><label for="quiet-hours" class="cursor-pointer">Тихие часы</label></div>
                {#if quietOn}
                  <div class="flex items-center gap-2 pl-6"><Input class="w-24" bind:value={quietStart} aria-label="Начало тихих часов" /><span class="text-muted-foreground">—</span><Input class="w-24" bind:value={quietEnd} aria-label="Конец тихих часов" /></div>
                {/if}
                <Button size="sm" disabled={busy || !alertsLoaded} onclick={saveAlerts}>Сохранить</Button>
                <details class="pt-2 text-xs text-muted-foreground">
                  <summary class="cursor-pointer">Текущие правила уведомлений</summary>
                  <pre class="mt-2 max-h-48 overflow-auto rounded-md border border-border bg-background p-2">{JSON.stringify(loaded?.alerts ?? {}, null, 2)}</pre>
                </details>
              </section>
            {:else if section === "widget"}
              <header class="mb-6 border-b border-border pb-4">
                <h3 class="text-lg font-semibold">Виджет</h3>
                <p class="mt-1 text-sm text-muted-foreground">Demerzel может показывать сводку проектов, задач и процессов в Центре уведомлений macOS.</p>
              </header>
              <div class="flex items-center gap-2">
                <Checkbox
                  id="widget-enabled"
                  checked={loaded?.widgetkit?.enabled ?? false}
                  onCheckedChange={async (value) => {
                    if (!loaded) return;
                    try { await saveSettings({ ...loaded, widgetkit: { enabled: value === true } }); }
                    catch (e) { error = String(e); }
                  }}
                />
                <label for="widget-enabled" class="cursor-pointer">Синхронизировать данные с виджетом</label>
              </div>
              <Button class="mt-4" size="sm" variant="outline" disabled={widgetBusy} onclick={() => void installWidgets()}>
                {widgetBusy ? "Устанавливаем…" : "Установить виджет в /Applications"}
              </Button>
              <p class="mt-2 text-xs text-muted-foreground">Для установки требуется сборка приложения для macOS с расширением WidgetKit.</p>
              {#if widgetMessage}<p class="mt-2 text-xs text-muted-foreground" role="status">{widgetMessage}</p>{/if}
            {:else if section === "planning"}
              <header class="mb-6 border-b border-border pb-4">
                <h3 class="text-lg font-semibold">Планирование</h3>
                <p class="mt-1 text-sm text-muted-foreground">Просмотр системных расписаний cron и launchd. Demerzel их не изменяет и не запускает.</p>
              </header>
              <div class="flex items-center gap-2">
                <Checkbox id="sys-schedules" checked={sysOn} onCheckedChange={(v) => void toggleSystemSchedules(v === true)} />
                <label for="sys-schedules" class="cursor-pointer">Показывать системные расписания</label>
              </div>
              {#if sysOn}
                {#if sysError}<p class="mt-3 text-xs text-destructive">{sysError}</p>
                {:else if sysJobs.length === 0}<p class="mt-3 text-sm text-muted-foreground">Расписаний не найдено.</p>
                {:else}
                  <ul class="mt-4 space-y-2">
                    {#each sysJobs as job (job.source + job.name)}
                      <li class="flex min-w-0 justify-between gap-3 rounded-md border border-border px-3 py-2 text-xs">
                        <span class="min-w-0 truncate font-mono" title={job.name}>{job.name}</span>
                        <span class="shrink-0 text-muted-foreground">{job.source}{job.schedule ? ` · ${job.schedule}` : ""}{job.pid !== null ? ` · PID ${job.pid}` : ""}{job.last_exit !== null ? ` · код ${job.last_exit}` : ""}</span>
                      </li>
                    {/each}
                  </ul>
                {/if}
              {/if}
            {:else if section === "calendar"}
              <header class="mb-6 border-b border-border pb-4">
                <h3 class="text-lg font-semibold">Календарь</h3>
                <p class="mt-1 text-sm text-muted-foreground">Demerzel записывает длительность процессов и AI-исправлений, а также доступные данные о токенах в файл .ics. Импортируйте его в календарь; файл обновляется после завершения работы.</p>
              </header>
              <Button size="sm" variant="outline" disabled={calendarBusy} onclick={() => void chooseCalendarFile()}>
                {loaded?.calendar?.export_path ? "Изменить файл .ics" : "Выбрать файл .ics"}
              </Button>
              {#if loaded?.calendar?.export_path}
                <p class="mt-2 break-all text-xs text-muted-foreground">{loaded.calendar.export_path}</p>
                <div class="mt-5 space-y-3 border-t border-border pt-4">
                  <div class="flex items-center gap-2">
                    <Checkbox id="calendar-processes" checked={loaded.calendar.include_processes} onCheckedChange={async (v) => { if (!loaded) return; try { await saveSettings({ ...loaded, calendar: { ...loaded.calendar!, include_processes: v === true } }); await invoke("export_calendar"); } catch (e) { error = String(e); } }} />
                    <label for="calendar-processes" class="cursor-pointer">Время работы процессов</label>
                  </div>
                  <div class="flex items-center gap-2">
                    <Checkbox id="calendar-agents" checked={loaded.calendar.include_agents} onCheckedChange={async (v) => { if (!loaded) return; try { await saveSettings({ ...loaded, calendar: { ...loaded.calendar!, include_agents: v === true } }); await invoke("export_calendar"); } catch (e) { error = String(e); } }} />
                    <label for="calendar-agents" class="cursor-pointer">Длительность исправлений и токены AI-агентов</label>
                  </div>
                  <Button size="sm" disabled={calendarBusy} onclick={() => void exportCalendarNow()}>
                    {calendarBusy ? "Обновляем…" : "Обновить файл сейчас"}
                  </Button>
                  <Button size="sm" variant="outline" onclick={() => void openCalendarFile()}>Открыть файл календаря</Button>
                </div>
              {/if}
              {#if calendarMessage}<p class="mt-3 text-xs text-muted-foreground" role="status">{calendarMessage}</p>{/if}
            {:else}
              <header class="mb-6 border-b border-border pb-4">
                <h3 class="text-lg font-semibold">О проекте</h3>
              </header>
              <div class="flex items-start gap-4">
                <img src="/mockups/demerzel_icon.png" alt="Demerzel" class="size-20 rounded-xl object-cover ring-1 ring-border" />
                <div class="space-y-2">
                  <p class="font-medium">Demerzel — тайный оркестратор локальных проектов</p>
                  <a class="inline-block text-primary underline underline-offset-4" href="https://github.com/korzhanov/demerzel" onclick={(event) => { event.preventDefault(); void openRepository(); }}>github.com/korzhanov/demerzel</a>
                  <p class="text-xs text-muted-foreground">{buildLabel()}</p>
                </div>
              </div>
              <p class="mt-5 text-muted-foreground">
                Управляйте рутиной из одного приложения. Demerzel собирает проекты, процессы, задачи и состояние портов в общей панели, чтобы вы быстрее замечали проблемы и понимали, что происходит в разработке.
              </p>
              <p class="mt-3 text-muted-foreground">
                Отслеживайте задачи GitHub и поручайте AI-агентам разбирать ошибки и готовить исправления.
              </p>
              <section class="mt-6 space-y-2 border-t border-border pt-4">
                <p class="font-medium">Возможности</p>
                <ul class="list-disc space-y-1 pl-5 text-muted-foreground">
                  <li>Запуск процессов разработки, мониторинг портов и обнаружение конфликтов.</li>
                  <li>Управление GitHub-проектами и задачами, работа с различными AI-агентами.</li>
                  <li>Живые уведомления и удобные автоматизации.</li>
                  <li>Виджеты macOS для быстрого мониторинга проектов.</li>
                </ul>
              </section>
              <section class="mt-6 space-y-2 border-t border-border pt-4">
                <p class="font-medium">Обновления Demerzel</p>
                <UpdateControl />
              </section>
            {/if}

            {#if message}<p class="mt-4 text-xs text-muted-foreground" role="status">{message}</p>{/if}
            {#if error}<p class="mt-4 text-xs text-destructive" role="alert">{error}</p>{/if}
          </div>
        </section>
      </div>
    </div>
  </div>
  {#if !settingsReady}<LoadingSplash label="Загружаем настройки…" />{/if}
{/if}
