<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { AutomationSelection, AutomationStatus, AutomationsView, RestartRule } from "$lib/types";
  import {
    shouldRestart,
    automationIsRunning,
    automationStatusText,
    initialSelection,
    isChecked,
    toggle,
  } from "$lib/automation";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";

  interface Props {
    path: string;
    name: string;
  }
  let { path, name }: Props = $props();

  let statuses = $state<AutomationStatus[]>([]);
  let restart = $state<RestartRule>({ enabled: false, max_retries: 3 });
  // Попытки перезапуска по имени и последний обработанный запуск (не даёт повторять один и тот же сбой).
  const attempts = new Map<string, number>();
  const handled = new Map<string, number | null>();

  async function setRestart(enabled: boolean) {
    restart = { ...restart, enabled };
    try {
      await invoke("save_restart_rule", { path, rule: restart });
    } catch (e) {
      actionError = String(e);
    }
  }

  async function applyRestartRules(list: AutomationStatus[]) {
    for (const s of list) {
      if (!s.command || s.run_id === null) continue;
      if (automationIsRunning(s)) {
        attempts.set(s.name, 0);
        continue;
      }
      if (handled.get(s.name) === s.run_id) continue;
      handled.set(s.name, s.run_id);
      const n = attempts.get(s.name) ?? 0;
      if (s.status && shouldRestart(s.status, n, restart)) {
        attempts.set(s.name, n + 1);
        try {
          await invoke("start_automation", { project: name, path, source: s.source, name: s.name });
        } catch (e) {
          actionError = String(e);
        }
      }
    }
  }
  let actionError = $state("");

  async function refreshStatuses() {
    const p = path;
    try {
      const s = await invoke<AutomationStatus[]>("automation_statuses", { path: p });
      if (p === path) {
        statuses = s;
        await applyRestartRules(s);
      }
    } catch {
      // статусы — вспомогательные; ошибку показывает загрузка списка
    }
  }

  $effect(() => {
    void path;
    statuses = [];
    attempts.clear();
    handled.clear();
    refreshStatuses();
    const timer = setInterval(refreshStatuses, 3000);
    return () => clearInterval(timer);
  });

  async function runAutomation(s: AutomationStatus) {
    actionError = "";
    try {
      if (automationIsRunning(s) && s.run_id !== null) {
        await invoke("stop_process", { id: s.run_id });
      } else if (s.command) {
        await invoke("start_automation", { project: name, path, source: s.source, name: s.name });
      }
    } catch (e) {
      actionError = String(e);
    }
    await refreshStatuses();
  }

  let view = $state<AutomationsView | null>(null);
  let selected = $state<AutomationSelection[]>([]);
  let error = $state("");
  let saved = $state(false);
  let saving = $state(false);

  $effect(() => {
    const p = path;
    view = null;
    error = "";
    saved = false;
    invoke<AutomationsView>("load_automations", { path: p })
      .then((v) => {
        if (p !== path) return;
        view = v;
        restart = v.restart;
        selected = initialSelection(v);
      })
      .catch((e) => {
        if (p === path) error = String(e);
      });
  });

  async function save() {
    saving = true;
    error = "";
    try {
      await invoke("save_automation_selection", { path, selected });
      saved = true;
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="space-y-2">
  <p class="text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">
    Автозадачи
  </p>
  {#if error}
    <p class="text-xs text-destructive">{error}</p>
  {:else if !view}
    <p class="text-xs text-muted-foreground">Поиск…</p>
  {:else if view.discovered.length === 0}
    <p class="text-xs text-muted-foreground">
      Скрипты не найдены. Добавьте <code>orchestrator.json</code> в папку проекта — и
      Demerzel будет знать, что запускать.
    </p>
  {:else}
    <ul class="space-y-1">
      {#each view.discovered as item (item.source + item.name)}
        <li class="flex min-w-0 items-start gap-2 text-sm">
          <Checkbox
            id={`auto-${item.source}-${item.name}`}
            checked={isChecked(selected, item)}
            onCheckedChange={() => (selected = toggle(selected, item))}
          />
          <label for={`auto-${item.source}-${item.name}`} class="min-w-0 cursor-pointer">
            <span class="font-mono text-xs">{item.name}</span>
            <span class="text-xs text-muted-foreground"> · {item.source}</span>
            {#if item.command}
              <span class="block truncate text-xs text-muted-foreground" title={item.command}
                >{item.command}</span
              >
            {/if}
          </label>
        </li>
      {/each}
    </ul>
    <div class="flex items-center gap-2">
      <Button size="sm" variant="outline" disabled={saving} onclick={save}>Сохранить</Button>
      {#if saved}<span class="text-xs text-muted-foreground">Сохранено</span>{/if}
    </div>
    {#if statuses.length > 0}
      <p class="pt-1 text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">
        Статус
      </p>
      <ul class="space-y-1">
        {#each statuses as s (s.source + s.name)}
          <li class="flex items-center justify-between gap-2 text-sm">
            <span class="min-w-0 truncate font-mono text-xs">{s.name}</span>
            <span class="flex shrink-0 items-center gap-2">
              <span class="text-xs text-muted-foreground">{automationStatusText(s)}</span>
              <Button
                size="sm"
                variant="outline"
                disabled={!s.command}
                onclick={() => runAutomation(s)}
                >{automationIsRunning(s) ? "Стоп" : "Запустить"}</Button
              >
            </span>
          </li>
        {/each}
      </ul>
      {#if actionError}<p class="text-xs text-destructive">{actionError}</p>{/if}
      <label class="flex cursor-pointer items-center gap-2 text-xs text-muted-foreground">
        <Checkbox
          checked={restart.enabled}
          onCheckedChange={(v) => setRestart(v === true)}
        />
        Перезапускать после сбоя (до {restart.max_retries} раз)
      </label>
    {/if}
  {/if}
</div>
