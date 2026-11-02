<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import type { Settings } from "$lib/types";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import type { SystemJob } from "$lib/types";
  import XIcon from "@lucide/svelte/icons/x";

  // windowed: окно самой программы (нативное окно настроек), без затемнения и с рамкой окна.
  let { open = $bindable(false), windowed = false }: { open: boolean; windowed?: boolean } = $props();

  type Tab = "general" | "advanced";
  let tab = $state<Tab>("general");
  let message = $state("");
  let error = $state("");
  let busy = $state(false);
  let preview = $state("");

  function close() {
    open = false;
    message = "";
    error = "";
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
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Настройки", extensions: ["json", "yaml", "yml"] }],
      });
      if (!picked || typeof picked !== "string") return;
      busy = true;
      const incoming = await invoke<Settings>("read_settings_file", { path: picked });
      await invoke("save_settings", { settings: incoming });
      message = "Настройки импортированы. Перезагрузите окно, чтобы всё применилось.";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Редкие параметры алертов (#138, вкладка «Дополнительно»). Значения по умолчанию — как в backend.
  let dedupe = $state(600);
  let voiceGap = $state(60);
  let quietOn = $state(false);
  let quietStart = $state("23:00");
  let quietEnd = $state("08:00");
  let timezone = $state("Europe/Berlin");
  let loaded = $state<Settings | null>(null);

  async function loadAdvanced() {
    try {
      const s = await invoke<Settings>("get_settings");
      loaded = s;
      sysOn = s.system_schedules?.enabled ?? false;
      if (sysOn) await refreshSystem();
      preview = JSON.stringify(s.alerts ?? {}, null, 2);
      dedupe = s.alerts?.dedupe_window_secs ?? 600;
      voiceGap = s.alerts?.voice_min_interval_secs ?? 60;
      quietOn = s.alerts?.quiet_hours?.enabled ?? false;
      quietStart = s.alerts?.quiet_hours?.start ?? "23:00";
      quietEnd = s.alerts?.quiet_hours?.end ?? "08:00";
      timezone = s.alerts?.timezone ?? "Europe/Berlin";
    } catch (e) {
      error = String(e);
    }
  }

  // Системные расписания (#153, этап E): по умолчанию выключено; только чтение.
  let sysOn = $state(false);
  let sysJobs = $state<SystemJob[]>([]);
  let sysError = $state("");

  async function toggleSystem(on: boolean) {
    sysError = "";
    try {
      const current = await invoke<Settings>("get_settings");
      const next: Settings = { ...current, system_schedules: { enabled: on } };
      await invoke("save_settings", { settings: next });
      sysOn = on;
      loaded = next;
      await refreshSystem();
    } catch (e) {
      sysError = String(e);
    }
  }

  async function refreshSystem() {
    sysError = "";
    try {
      sysJobs = await invoke<SystemJob[]>("system_schedules");
    } catch (e) {
      sysError = String(e);
    }
  }

  const HHMM = /^([01]\d|2[0-3]):[0-5]\d$/;

  async function saveAdvanced() {
    error = "";
    message = "";
    if (!loaded) return;
    if (!Number.isInteger(dedupe) || dedupe < 0 || !Number.isInteger(voiceGap) || voiceGap < 0) {
      error = "Интервалы — целые секунды, не меньше нуля.";
      return;
    }
    if (quietOn && (!HHMM.test(quietStart) || !HHMM.test(quietEnd))) {
      error = "Тихие часы — в формате ЧЧ:ММ, например 23:00.";
      return;
    }
    busy = true;
    try {
      const next: Settings = {
        ...loaded,
        alerts: {
          ...(loaded.alerts ?? { rules: [] }),
          dedupe_window_secs: dedupe,
          voice_min_interval_secs: voiceGap,
          quiet_hours: { enabled: quietOn, start: quietStart, end: quietEnd },
          timezone: timezone.trim() || "Europe/Berlin",
        },
      };
      await invoke("save_settings", { settings: next });
      loaded = next;
      message = "Сохранено.";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (open && tab === "advanced") loadAdvanced();
  });
</script>

{#if open}
  <div
    class={windowed ? "flex min-h-screen flex-col" : "fixed inset-0 z-40 flex items-center justify-center bg-black/45 p-3"}
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) close();
    }}
  >
    <div
      class={windowed
        ? "flex min-h-screen w-full flex-col bg-card text-card-foreground"
        : "flex max-h-[min(90vh,42rem)] w-[min(34rem,calc(100vw-1.5rem))] flex-col overflow-hidden rounded-lg border border-border bg-card text-card-foreground shadow-lg"}
      role="dialog"
      aria-modal="true"
      aria-labelledby="app-settings-title"
    >
      <div class="flex items-center justify-between border-b border-border px-4 py-3">
        <h2 id="app-settings-title" class="text-base font-medium">Настройки</h2>
        <Button size="sm" variant="ghost" class="size-8 p-0" aria-label="Закрыть" onclick={close}>
          <XIcon class="size-4" />
        </Button>
      </div>

      <div class="flex gap-1 border-b border-border px-4 pt-2" role="tablist">
        <button
          role="tab"
          aria-selected={tab === "general"}
          class="border-b-2 px-2 pb-2 text-sm {tab === 'general' ? 'border-primary font-medium' : 'border-transparent text-muted-foreground'}"
          onclick={() => (tab = "general")}>Основные</button
        >
        <button
          role="tab"
          aria-selected={tab === "advanced"}
          class="border-b-2 px-2 pb-2 text-sm {tab === 'advanced' ? 'border-primary font-medium' : 'border-transparent text-muted-foreground'}"
          onclick={() => (tab = "advanced")}>Дополнительно</button
        >
      </div>

      <div class="space-y-4 overflow-y-auto p-4 text-sm">
        {#if tab === "general"}
          <section class="space-y-2">
            <p class="font-medium">Файл настроек</p>
            <p class="text-muted-foreground">
              Экспорт и импорт в JSON или YAML. Импорт заменяет текущие настройки.
            </p>
            <div class="flex flex-wrap gap-2">
              <Button size="sm" variant="outline" disabled={busy} onclick={exportSettings}
                >Экспорт…</Button
              >
              <Button size="sm" variant="outline" disabled={busy} onclick={importSettings}
                >Импорт…</Button
              >
            </div>
          </section>
        {:else}
          <section class="space-y-3">
            <p class="font-medium">Алерты: редко меняемое</p>
            <label class="flex items-center justify-between gap-3">
              <span>Не повторять одно и то же событие, сек</span>
              <Input class="w-28" type="number" min="0" bind:value={dedupe} />
            </label>
            <label class="flex items-center justify-between gap-3">
              <span>Минимум между голосовыми алертами, сек</span>
              <Input class="w-28" type="number" min="0" bind:value={voiceGap} />
            </label>
            <label class="flex items-center justify-between gap-3">
              <span>Часовой пояс</span>
              <Input class="w-44" bind:value={timezone} />
            </label>
            <div class="flex items-center gap-2">
              <Checkbox id="quiet-on" checked={quietOn} onCheckedChange={(v) => (quietOn = v === true)} />
              <label for="quiet-on" class="cursor-pointer">Тихие часы</label>
            </div>
            {#if quietOn}
              <div class="flex items-center gap-2 pl-6">
                <Input class="w-24" bind:value={quietStart} aria-label="Начало тихих часов" />
                <span class="text-muted-foreground">—</span>
                <Input class="w-24" bind:value={quietEnd} aria-label="Конец тихих часов" />
              </div>
            {/if}
            <div>
              <Button size="sm" disabled={busy || !loaded} onclick={saveAdvanced}>Сохранить</Button>
            </div>
            <div class="space-y-2 border-t border-border pt-3">
              <p class="font-medium">Системные расписания (только чтение)</p>
              <p class="text-xs text-muted-foreground">
                Показывает crontab и launchd (без задач Apple). Demerzel ничего в них не меняет и не запускает.
              </p>
              <div class="flex items-center gap-2">
                <Checkbox id="sys-on" checked={sysOn} onCheckedChange={(v) => toggleSystem(v === true)} />
                <label for="sys-on" class="cursor-pointer">Показывать системные расписания</label>
              </div>
              {#if sysOn}
                {#if sysError}
                  <p class="text-xs text-destructive">{sysError}</p>
                {:else if sysJobs.length === 0}
                  <p class="text-xs text-muted-foreground">Ничего не найдено.</p>
                {:else}
                  <ul class="max-h-40 space-y-1 overflow-auto text-xs">
                    {#each sysJobs as job (job.source + job.name)}
                      <li class="flex min-w-0 justify-between gap-2">
                        <span class="min-w-0 truncate font-mono" title={job.name}>{job.name}</span>
                        <span class="shrink-0 text-muted-foreground">
                          {job.source}{job.schedule ? ` · ${job.schedule}` : ""}{job.pid !== null ? ` · PID ${job.pid}` : ""}{job.last_exit !== null ? ` · код ${job.last_exit}` : ""}
                        </span>
                      </li>
                    {/each}
                  </ul>
                {/if}
              {/if}
            </div>
            <details class="text-xs text-muted-foreground">
              <summary class="cursor-pointer">Сырой JSON алертов (для продвинутых)</summary>
              <pre class="mt-2 max-h-64 overflow-auto rounded-md border border-border bg-background p-2">{preview}</pre>
            </details>
          </section>
        {/if}

        {#if message}<p class="text-xs text-muted-foreground">{message}</p>{/if}
        {#if error}<p class="text-xs text-destructive">{error}</p>{/if}
      </div>
    </div>
  </div>
{/if}
