<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import type { Settings } from "$lib/types";
  import { Button } from "$lib/components/ui/button/index.js";
  import XIcon from "@lucide/svelte/icons/x";

  let { open = $bindable(false) }: { open: boolean } = $props();

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

  async function loadPreview() {
    try {
      const s = await invoke<Settings>("get_settings");
      preview = JSON.stringify(s.alerts ?? {}, null, 2);
    } catch (e) {
      preview = String(e);
    }
  }

  $effect(() => {
    if (open && tab === "advanced") loadPreview();
  });
</script>

{#if open}
  <div
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/45 p-3"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) close();
    }}
  >
    <div
      class="flex max-h-[min(90vh,42rem)] w-[min(34rem,calc(100vw-1.5rem))] flex-col overflow-hidden rounded-lg border border-border bg-card text-card-foreground shadow-lg"
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
          <section class="space-y-2">
            <p class="font-medium">Редко используемое</p>
            <p class="text-muted-foreground">
              Здесь будут настройки, которые нужны не каждый день. Сейчас показан только раздел алертов
              в том виде, как он хранится.
            </p>
            <pre class="max-h-64 overflow-auto rounded-md border border-border bg-background p-2 text-xs">{preview}</pre>
          </section>
        {/if}

        {#if message}<p class="text-xs text-muted-foreground">{message}</p>{/if}
        {#if error}<p class="text-xs text-destructive">{error}</p>{/if}
      </div>
    </div>
  </div>
{/if}
