<script lang="ts">
  import { onMount } from "svelte";
  import { isTauri } from "@tauri-apps/api/core";
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { Button } from "$lib/components/ui/button/index.js";

  type PendingUpdate = Awaited<ReturnType<typeof check>>;

  let runningInTauri = $state(false);
  let pendingUpdate = $state<PendingUpdate>(null);
  let checking = $state(false);
  let installing = $state(false);
  let progress = $state<number | null>(null);
  let message = $state("");
  let error = $state("");

  async function checkForUpdates(silent = false) {
    if (!isTauri() || checking || installing) return;
    checking = true;
    error = "";
    if (!silent) message = "";

    try {
      const next = await check();
      if (pendingUpdate) await pendingUpdate.close();
      pendingUpdate = next;
      if (!next && !silent) message = "Установлена актуальная версия.";
    } catch (cause) {
      if (silent) console.warn("Demerzel update check failed:", cause);
      else error = `Не удалось проверить обновления: ${String(cause)}`;
    } finally {
      checking = false;
    }
  }

  async function installUpdate() {
    const update = pendingUpdate;
    if (!update || installing) return;

    installing = true;
    progress = 0;
    error = "";
    message = "Загрузка обновления…";
    let downloadedBytes = 0;
    let contentLength: number | null = null;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          contentLength = event.data.contentLength ?? null;
          progress = contentLength ? 0 : null;
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          progress = contentLength
            ? Math.min(99, Math.round((downloadedBytes / contentLength) * 100))
            : null;
        } else if (event.event === "Finished") {
          progress = 100;
        }
      });
      message = "Обновление установлено. Перезапускаю Demerzel…";
      await relaunch();
    } catch (cause) {
      error = `Не удалось установить обновление: ${String(cause)}`;
      message = "";
      installing = false;
    }
  }

  onMount(() => {
    runningInTauri = isTauri();
    if (!runningInTauri) return;
    const startupCheck = window.setTimeout(() => void checkForUpdates(true), 8_000);
    const periodicCheck = window.setInterval(() => void checkForUpdates(true), 6 * 60 * 60 * 1_000);
    return () => {
      window.clearTimeout(startupCheck);
      window.clearInterval(periodicCheck);
      void pendingUpdate?.close();
    };
  });
</script>

{#if runningInTauri}
  <Button
    size="sm"
    variant="outline"
    class="h-8 px-2 text-xs"
    disabled={checking || installing}
    aria-label="Проверить обновления Demerzel"
    onclick={() => void checkForUpdates()}
  >
    {checking ? "Проверка…" : "Обновления"}
  </Button>
{/if}

{#if runningInTauri && pendingUpdate}
  <div
    class="mx-4 mt-2 flex flex-wrap items-center gap-2 rounded-md border border-primary/30 bg-primary/5 px-3 py-2 text-sm"
    role="status"
    aria-live="polite"
  >
    <span class="min-w-0 flex-1">Demerzel {pendingUpdate.version} доступна для установки.</span>
    {#if pendingUpdate.body}
      <span class="w-full text-xs text-muted-foreground">{pendingUpdate.body}</span>
    {/if}
    <Button size="sm" disabled={installing} onclick={() => void installUpdate()}>
      {#if installing}
        {progress === null ? "Загрузка…" : `Загрузка ${progress}%`}
      {:else}
        Установить и перезапустить
      {/if}
    </Button>
  </div>
{/if}

{#if runningInTauri && message}
  <p class="px-4 pt-2 text-sm text-muted-foreground" role="status">{message}</p>
{/if}
{#if runningInTauri && error}
  <p class="break-words px-4 pt-2 text-sm text-destructive" role="alert">{error}</p>
{/if}
