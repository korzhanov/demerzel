<script lang="ts">
  // Нативное окно настроек (#settings-window): отдельное окно Tauri, маршрут /settings.
  import { invoke } from "@tauri-apps/api/core";
  import AppSettingsDialog from "$lib/components/AppSettingsDialog.svelte";

  let open = $state(true);

  $effect(() => {
    if (!open) void invoke("close_settings_window");
  });
</script>

<div class="min-h-screen bg-background text-foreground">
  <AppSettingsDialog bind:open windowed />
</div>
