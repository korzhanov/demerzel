<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { AutomationSelection, AutomationsView } from "$lib/types";
  import { initialSelection, isChecked, toggle } from "$lib/automation";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";

  interface Props {
    path: string;
  }
  let { path }: Props = $props();

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
  {/if}
</div>
