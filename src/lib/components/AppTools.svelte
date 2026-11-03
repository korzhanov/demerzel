<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AgentCatalogEntry, Settings } from "$lib/types";
  import { AGENT_RUNNERS, parseAgentRunner } from "$lib/types";
  import { mergeCatalogPrefs, type AgentCatalogPref } from "$lib/agent-install";
  import { Button } from "$lib/components/ui/button/index.js";
  import AgentsSettingsDialog from "$lib/components/AgentsSettingsDialog.svelte";
  import UpdateControl from "$lib/components/UpdateControl.svelte";

  let agentsOpen = $state(false);
  let agentCatalog = $state<AgentCatalogEntry[]>([]);
  let agentCatalogPrefs = $state<AgentCatalogPref[]>([]);

  let widgetBusy = $state(false);
  let widgetMsg = $state("");
  let notificationBusy = $state(false);
  let notificationMsg = $state("");
  let toolError = $state("");

  onMount(() => {
    void loadAgents();
  });

  async function loadAgents() {
    try {
      const s = await invoke<Settings>("get_settings");
      agentCatalogPrefs = mergeCatalogPrefs(
        (s.agents?.catalog ?? []).map((p) => ({
          runner: parseAgentRunner(p.runner),
          enabled: p.enabled !== false,
        })),
        AGENT_RUNNERS,
      );
      agentCatalog = await invoke<AgentCatalogEntry[]>("list_available_agents");
    } catch (e) {
      toolError = String(e);
    }
  }

  async function savePrefs(next: AgentCatalogPref[]) {
    agentCatalogPrefs = next;
    try {
      const current = await invoke<Settings>("get_settings");
      await invoke("save_settings", {
        settings: {
          ...current,
          agents: { ...(current.agents ?? {}), catalog: next },
        } as Settings,
      });
    } catch (e) {
      toolError = String(e);
    }
  }

  async function installWidgets() {
    widgetBusy = true;
    widgetMsg = "";
    toolError = "";
    try {
      widgetMsg = await invoke<string>("install_widgets");
    } catch (e) {
      toolError = String(e);
    } finally {
      widgetBusy = false;
    }
  }

  async function testNotifications() {
    notificationBusy = true;
    notificationMsg = "";
    try {
      const result = await invoke<{
        notified: boolean;
        spoken: boolean;
        speak_error: string | null;
      }>("test_notification");
      notificationMsg = result.spoken
        ? "Уведомление и голос запущены"
        : `Уведомление запущено · голос: ${result.speak_error ?? "не запущен"}`;
    } catch (e) {
      notificationMsg = `Ошибка уведомления: ${String(e)}`;
    } finally {
      notificationBusy = false;
    }
  }
</script>

<section class="space-y-3">
  <p class="font-medium">Инструменты</p>
  <div class="flex flex-wrap gap-2">
    <Button size="sm" variant="outline" disabled={notificationBusy} onclick={() => void testNotifications()}
      >Проверить уведомление и голос</Button
    >
    <Button size="sm" variant="outline" disabled={widgetBusy} onclick={() => void installWidgets()}
      >Установить виджеты в /Applications</Button
    >
    <Button size="sm" variant="outline" onclick={() => (agentsOpen = true)}
      >Агенты: приоритет и установка…</Button
    >
  </div>
  <p class="text-xs text-muted-foreground">Установка виджетов нужна сборка через tauri build --debug.</p>
  {#if notificationMsg}<p class="text-xs text-muted-foreground" role="status">{notificationMsg}</p>{/if}
  {#if widgetMsg}<p class="text-xs text-[var(--status-ok)]" role="status">{widgetMsg}</p>{/if}
  {#if toolError}<p class="text-xs text-destructive">{toolError}</p>{/if}

  <div class="space-y-2 border-t border-border pt-3">
    <p class="font-medium">Обновления</p>
    <UpdateControl />
  </div>
</section>

<AgentsSettingsDialog
  bind:open={agentsOpen}
  bind:catalog={agentCatalog}
  bind:prefs={agentCatalogPrefs}
  onPersist={savePrefs}
/>
