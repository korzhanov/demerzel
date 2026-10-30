<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy, onMount } from "svelte";
  import type { Settings } from "$lib/types";
  import {
    isRunning,
    statusLabel,
    type LogLine,
    type Project,
    type ProcessRow,
  } from "$lib/types";

  let roots = $state("");
  let projects = $state<Project[]>([]);
  let processes = $state<ProcessRow[]>([]);
  let selectedId = $state<number | null>(null);
  let logs = $state<LogLine[]>([]);
  /** Переопределение: если пусто — берём commands.dev / scripts.dev с диска. */
  let commandOverride = $state("");
  let error = $state("");

  let poll: ReturnType<typeof setInterval> | undefined;

  function rootList(): string[] {
    return roots
      .split("\n")
      .map((r) => r.trim())
      .filter(Boolean);
  }

  async function saveRoots() {
    try {
      await invoke("save_settings", { settings: { scan_roots: rootList() } as Settings });
    } catch (e) {
      error = String(e);
    }
  }

  async function scan() {
    error = "";
    await saveRoots();
    try {
      projects = await invoke<Project[]>("scan_projects", { roots: rootList() });
    } catch (e) {
      error = String(e);
    }
  }

  async function refresh() {
    try {
      processes = await invoke<ProcessRow[]>("list_processes");
      if (selectedId !== null) {
        const l = await invoke<LogLine[] | null>("process_logs", { id: selectedId });
        logs = l ?? [];
      }
    } catch (e) {
      error = String(e);
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

  onMount(async () => {
    try {
      const s = await invoke<Settings>("get_settings");
      roots = s.scan_roots.join("\n");
      if (roots) await scan();
    } catch (e) {
      error = String(e);
    }
    refresh();
    poll = setInterval(refresh, 1000);
  });

  onDestroy(() => {
    if (poll) clearInterval(poll);
  });
</script>

{#snippet tree(list: Project[], depth: number)}
  {#each list as p (p.path)}
    <li style="padding-left: {depth * 16}px">
      <span>{p.name}</span>
      <button type="button" onclick={() => start(p)}>запустить</button>
    </li>
    {#if p.children.length}
      {@render tree(p.children, depth + 1)}
    {/if}
  {/each}
{/snippet}

<main>
  <header>
    <h1>Demerzel</h1>
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <section>
    <h2>Корневые папки</h2>
    <textarea bind:value={roots} rows="2" placeholder="/Users/you/code"></textarea>
    <div class="row">
      <button type="button" onclick={scan}>Сканировать</button>
      <label
        >команда (пусто = из orchestrator.json / package.json):
        <input bind:value={commandOverride} placeholder="авто" />
      </label>
    </div>
    <ul class="tree">
      {@render tree(projects, 0)}
    </ul>
  </section>

  <section>
    <h2>Процессы</h2>
    {#if processes.length === 0}
      <p>Нет запущенных процессов.</p>
    {:else}
      <table>
        <thead>
          <tr><th>проект</th><th>команда</th><th>статус</th><th></th></tr>
        </thead>
        <tbody>
          {#each processes as row (row.id)}
            <tr class:selected={row.id === selectedId} onclick={() => select(row.id)}>
              <td>{row.project}</td>
              <td>{row.command}</td>
              <td>{statusLabel(row.status)}</td>
              <td>
                {#if isRunning(row.status)}
                  <button
                    type="button"
                    onclick={(e) => {
                      e.stopPropagation();
                      stop(row.id);
                    }}>стоп</button
                  >
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <section>
    <h2>Логи {selectedId !== null ? `#${selectedId}` : ""}</h2>
    <pre class="logs">{#each logs as line, i (i)}<span class:err={line.stream === "stderr"}>{line.text}
</span>{/each}</pre>
  </section>
</main>

<style>
  main { font-family: system-ui, sans-serif; padding: 16px; max-width: 960px; margin: 0 auto; }
  section { margin-bottom: 24px; }
  .row { display: flex; gap: 8px; align-items: center; margin: 8px 0; flex-wrap: wrap; }
  textarea, input { width: 100%; font: inherit; }
  label { flex: 1; min-width: 240px; }
  .tree { list-style: none; padding: 0; }
  .tree li { display: flex; gap: 8px; align-items: center; padding: 4px 0; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid #ddd; }
  tr.selected { background: #f0f0ff; }
  .logs { background: #0d0d12; color: #ddd; padding: 12px; min-height: 120px; max-height: 320px; overflow: auto; }
  .err { color: #ff7b72; }
  .error { color: #b00020; }
</style>
