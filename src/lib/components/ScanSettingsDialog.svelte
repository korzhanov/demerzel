<script lang="ts">
  import { untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import type { AutofixMode } from "$lib/types";
  import { AUTOFIX_MODES, parseAutofixMode } from "$lib/types";
  import { autofixModeLabel } from "$lib/agents";
  import {
    allEnabled,
    enabledRootPaths,
    hasCustomRules,
    noneEnabled,
    normalizePath,
    removeEntry,
    setAllEnabled,
    upsertEntry,
    type DirChild,
    type OrchestrationStatus,
    type ScanRootEntry,
    type ScanRootRules,
  } from "$lib/scan-roots";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import FolderPlusIcon from "@lucide/svelte/icons/folder-plus";
  import FolderTreeIcon from "@lucide/svelte/icons/folder-tree";
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
  import Settings2Icon from "@lucide/svelte/icons/settings-2";
  import Trash2Icon from "@lucide/svelte/icons/trash-2";
  import WandSparklesIcon from "@lucide/svelte/icons/wand-sparkles";
  import ScanSearchIcon from "@lucide/svelte/icons/scan-search";
  import CircleCheckIcon from "@lucide/svelte/icons/circle-check";
  import CircleAlertIcon from "@lucide/svelte/icons/circle-alert";
  import XIcon from "@lucide/svelte/icons/x";
  import CheckCheckIcon from "@lucide/svelte/icons/check-check";
  import SquareIcon from "@lucide/svelte/icons/square";

  let {
    open = $bindable(false),
    entries = $bindable<ScanRootEntry[]>([]),
    autofix = $bindable<AutofixMode>("off"),
    scanBusy = false,
    onScan,
    onAutofixChange,
    onPersist,
  }: {
    open?: boolean;
    entries?: ScanRootEntry[];
    autofix?: AutofixMode;
    scanBusy?: boolean;
    onScan: () => void | Promise<void>;
    onAutofixChange: (mode: AutofixMode) => void | Promise<void>;
    onPersist: (entries: ScanRootEntry[]) => void | Promise<void>;
  } = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let expanded = $state<Record<string, boolean>>({});
  let childrenCache = $state<Record<string, DirChild[]>>({});
  let orchCache = $state<Record<string, OrchestrationStatus>>({});
  let loadingPath = $state<string | null>(null);
  let rulesPath = $state<string | null>(null);
  let pickBusy = $state(false);
  let error = $state("");
  const selectAllState = $derived(
    allEnabled(entries) ? true : noneEnabled(entries) ? false : "indeterminate",
  );
  const enabledCount = $derived(enabledRootPaths(entries).length);

  $effect(() => {
    const el = dialogEl;
    if (!el) return;
    if (open) {
      if (!el.open) el.showModal();
    } else if (el.open) {
      el.close();
    }
  });

  $effect(() => {
    if (!open) return;
    const paths = entries.map((e) => normalizePath(e.path));
    void refreshOrchestration(paths);
  });

  function onDialogClose() {
    open = false;
    rulesPath = null;
  }

  async function persist(next: ScanRootEntry[]) {
    entries = next;
    await onPersist(next);
  }

  async function refreshOrchestration(paths: string[]) {
    const updates: Record<string, OrchestrationStatus> = {};
    await Promise.all(
      paths.map(async (p) => {
        if (!p) return;
        try {
          const status = await invoke<string>("get_orchestration", { path: p });
          updates[p] = status === "configured" ? "configured" : "missing";
        } catch {
          updates[p] = "unknown";
        }
      }),
    );
    orchCache = { ...untrack(() => orchCache), ...updates };
  }

  async function pickFolders() {
    pickBusy = true;
    error = "";
    try {
      const picked = await openDialog({
        directory: true,
        multiple: true,
        title: "Корневые папки для сканирования",
      });
      if (picked == null) return;
      const paths = Array.isArray(picked) ? picked : [picked];
      let next = [...entries];
      for (const p of paths) {
        next = upsertEntry(next, p, { enabled: true });
      }
      await persist(next);
      await refreshOrchestration(paths.map(normalizePath));
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      pickBusy = false;
    }
  }

  async function ensureChildren(path: string) {
    const key = normalizePath(path);
    if (childrenCache[key]) return;
    loadingPath = key;
    try {
      const kids = await invoke<DirChild[]>("list_dir_children", { path: key });
      childrenCache = { ...childrenCache, [key]: kids };
      const orchNext = { ...orchCache };
      for (const k of kids) {
        orchNext[normalizePath(k.path)] = k.orchestration;
      }
      orchCache = orchNext;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      childrenCache = { ...childrenCache, [key]: [] };
    } finally {
      loadingPath = null;
    }
  }

  async function toggleExpand(path: string) {
    const key = normalizePath(path);
    const willOpen = !expanded[key];
    expanded = { ...expanded, [key]: willOpen };
    if (willOpen) await ensureChildren(key);
  }

  async function setEnabled(path: string, enabled: boolean) {
    await persist(upsertEntry(entries, path, { enabled }));
  }

  async function toggleSelectAll(wantAll: boolean) {
    await persist(setAllEnabled(entries, wantAll));
  }

  async function removeRoot(path: string) {
    await persist(removeEntry(entries, path));
    const key = normalizePath(path);
    const { [key]: _e, ...restExp } = expanded;
    expanded = restExp;
  }

  async function saveRules(path: string, rules: ScanRootRules | null) {
    await persist(upsertEntry(entries, path, { rules }));
    rulesPath = null;
  }

  async function ensureOrchestration(path: string) {
    error = "";
    const key = normalizePath(path);
    try {
      await invoke<string>("init_orchestrator", { path: key });
      orchCache = { ...orchCache, [key]: "configured" };
      for (const [parent, kids] of Object.entries(childrenCache)) {
        if (!kids.some((c) => normalizePath(c.path) === key)) continue;
        childrenCache = {
          ...childrenCache,
          [parent]: kids.map((c) =>
            normalizePath(c.path) === key ? { ...c, orchestration: "configured" } : c,
          ),
        };
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function openRules(path: string) {
    const key = normalizePath(path);
    if (!entryFor(key)) {
      await persist(upsertEntry(entries, key, { enabled: false }));
    }
    rulesPath = key;
  }

  async function scanAndClose() {
    await onScan();
    open = false;
  }

  function entryFor(path: string): ScanRootEntry | undefined {
    return entries.find((e) => normalizePath(e.path) === normalizePath(path));
  }

  function orchOf(path: string): OrchestrationStatus {
    return orchCache[normalizePath(path)] ?? "unknown";
  }

  function basename(path: string): string {
    const n = normalizePath(path);
    const i = n.lastIndexOf("/");
    return i >= 0 ? n.slice(i + 1) || n : n;
  }

  function orchLabel(orch: OrchestrationStatus): string {
    if (orch === "configured") return "Оркестрация настроена";
    if (orch === "missing") return "Оркестрация не настроена — можно установить";
    return "Статус оркестрации неизвестен";
  }
</script>

{#snippet treeNode(nodePath: string, label: string, depth: number, isRoot: boolean)}
  {@const key = normalizePath(nodePath)}
  {@const entry = entryFor(key)}
  {@const isOpen = !!expanded[key]}
  {@const kids = childrenCache[key]}
  {@const orch = orchOf(key)}
  {@const checked = entry?.enabled ?? false}
  <li class="rounded-md">
    <div
      class="flex min-w-0 items-center gap-0.5 rounded-md px-1 py-0.5 hover:bg-accent/40"
      style:padding-left="{4 + depth * 12}px"
    >
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              type="button"
              size="sm"
              variant="ghost"
              class="size-7 shrink-0 p-0"
              aria-expanded={isOpen}
              aria-label={isOpen ? "Свернуть" : "Развернуть"}
              onclick={() => void toggleExpand(nodePath)}
            >
              {#if isOpen}
                <ChevronDownIcon class="size-4" />
              {:else}
                <ChevronRightIcon class="size-4" />
              {/if}
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content portalDisabled to={dialogEl ?? undefined}
          >{isOpen ? "Свернуть" : "Развернуть"}</Tooltip.Content
        >
      </Tooltip.Root>

      <Checkbox
        checked={checked}
        onCheckedChange={(v) => void setEnabled(nodePath, v === true)}
        aria-label={`Сканировать ${label}`}
      />

      <span class="min-w-0 flex-1 truncate font-mono text-xs" title={nodePath}>{label}</span>

      {#if hasCustomRules(entry)}
        <span
          class="rounded bg-primary/15 px-1 py-0.5 text-[10px] text-primary"
          title="Свои правила скана">R</span
        >
      {/if}

      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <span {...props} class="inline-flex" aria-label={orchLabel(orch)}>
              {#if orch === "configured"}
                <CircleCheckIcon class="size-3.5 text-[var(--status-ok)]" aria-hidden="true" />
              {:else}
                <CircleAlertIcon
                  class={[
                    "size-3.5",
                    orch === "missing" ? "text-primary" : "text-muted-foreground",
                  ]}
                  aria-hidden="true"
                />
              {/if}
            </span>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>{orchLabel(orch)}</Tooltip.Content>
      </Tooltip.Root>

      {#if orch === "missing"}
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <Button
                {...props}
                type="button"
                size="sm"
                variant="outline"
                class="h-7 gap-1 px-2"
                aria-label="Установить оркестрацию"
                onclick={() => void ensureOrchestration(nodePath)}
              >
                <WandSparklesIcon class="size-3.5" />
              </Button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>Создать orchestrator.json</Tooltip.Content>
        </Tooltip.Root>
      {/if}

      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              type="button"
              size="sm"
              variant="ghost"
              class="size-7 p-0"
              aria-label="Правила папки"
              onclick={() => void openRules(nodePath)}
            >
              <Settings2Icon class="size-3.5" />
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>Правила скана (приоритет над предком)</Tooltip.Content>
      </Tooltip.Root>

      {#if isRoot}
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <Button
                {...props}
                type="button"
                size="sm"
                variant="ghost"
                class="size-7 p-0 text-destructive"
                aria-label="Убрать корень"
                onclick={() => void removeRoot(nodePath)}
              >
                <Trash2Icon class="size-3.5" />
              </Button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>Убрать из списка</Tooltip.Content>
        </Tooltip.Root>
      {/if}
    </div>

    {#if isOpen}
      <ul class="space-y-0.5 border-l border-border/70" style:margin-left="{16 + depth * 12}px">
        {#if loadingPath === key}
          <li class="px-2 py-1 text-xs text-muted-foreground">Загрузка…</li>
        {:else if !kids || kids.length === 0}
          <li class="px-2 py-1 text-xs text-muted-foreground">Нет подпапок</li>
        {:else}
          {#each kids as kid (kid.path)}
            {@render treeNode(kid.path, kid.name, depth + 1, false)}
          {/each}
        {/if}
      </ul>
    {/if}
  </li>
{/snippet}

<dialog
  bind:this={dialogEl}
  class="fixed left-1/2 top-1/2 z-50 w-[min(36rem,calc(100vw-1.5rem))] max-h-[min(90vh,40rem)] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-lg border border-border bg-card p-0 text-card-foreground shadow-lg backdrop:bg-black/55"
  aria-labelledby="scan-settings-title"
  onclose={onDialogClose}
  onclick={(e) => {
    if (e.target === dialogEl) open = false;
  }}
>
  <Tooltip.Provider>
    <div class="flex max-h-[min(90vh,40rem)] flex-col">
      <header class="flex items-start gap-2 border-b border-border px-4 py-3">
        <div class="min-w-0 flex-1 space-y-1">
          <h2
            id="scan-settings-title"
            class="flex items-center gap-2 text-base font-medium tracking-tight"
          >
            <FolderTreeIcon class="size-4 text-primary" aria-hidden="true" />
            Корни сканирования
          </h2>
          <p class="text-sm text-muted-foreground">
            Dirpicker · дерево · чекбоксы. Вложенные правила важнее предков.
          </p>
        </div>
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <Button
                {...props}
                type="button"
                size="sm"
                variant="ghost"
                class="size-8 p-0"
                aria-label="Закрыть"
                onclick={() => (open = false)}
              >
                <XIcon class="size-4" />
              </Button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>Закрыть</Tooltip.Content>
        </Tooltip.Root>
      </header>

      <div class="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-3">
        <div class="flex flex-wrap items-center gap-2">
          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  type="button"
                  size="sm"
                  class="gap-1.5"
                  disabled={pickBusy}
                  aria-label="Добавить папки"
                  onclick={() => void pickFolders()}
                >
                  <FolderPlusIcon class="size-4" />
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>Добавить папки через системный dirpicker</Tooltip.Content>
          </Tooltip.Root>

          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  type="button"
                  size="sm"
                  variant="outline"
                  class="size-8 p-0"
                  aria-label="Выбрать всё"
                  disabled={entries.length === 0 || selectAllState === true}
                  onclick={() => void toggleSelectAll(true)}
                >
                  <CheckCheckIcon class="size-4" />
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>Выбрать все корни</Tooltip.Content>
          </Tooltip.Root>

          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  type="button"
                  size="sm"
                  variant="outline"
                  class="size-8 p-0"
                  aria-label="Снять всё"
                  disabled={entries.length === 0 || selectAllState === false}
                  onclick={() => void toggleSelectAll(false)}
                >
                  <SquareIcon class="size-4" />
                </Button>
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>Снять все корни</Tooltip.Content>
          </Tooltip.Root>

          <Checkbox
            checked={selectAllState === true}
            indeterminate={selectAllState === "indeterminate"}
            onCheckedChange={(v) => void toggleSelectAll(v === true)}
            aria-label="Выбрать всё или ничего"
          />

          <div class="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground tabular-nums">
            {enabledCount}/{entries.length}
          </div>
        </div>

        {#if error}
          <p class="text-sm text-destructive" role="alert">{error}</p>
        {/if}

        {#if entries.length === 0}
          <p
            class="rounded-md border border-dashed border-border px-3 py-6 text-center text-sm text-muted-foreground"
          >
            Нет корней — нажми «+» и выбери папки с проектами.
          </p>
        {:else}
          <ul class="space-y-0.5 rounded-md border border-border p-1">
            {#each entries as entry (entry.path)}
              {@render treeNode(entry.path, entry.path, 0, true)}
            {/each}
          </ul>
        {/if}

        <label class="flex min-w-0 flex-col gap-1.5 text-sm">
          <span class="text-muted-foreground">Автофикс по алерту</span>
          <select
            class="h-9 w-full rounded-md border border-input bg-background px-3 text-sm text-foreground"
            aria-label="Режим автофикса по алерту"
            value={autofix}
            onchange={(e) => {
              void onAutofixChange(
                parseAutofixMode((e.currentTarget as HTMLSelectElement).value),
              );
            }}
          >
            {#each AUTOFIX_MODES as mode (mode)}
              <option value={mode}>{autofixModeLabel(mode)}</option>
            {/each}
          </select>
        </label>
      </div>

      <footer class="flex flex-wrap justify-end gap-2 border-t border-border px-4 py-3">
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <Button
                {...props}
                type="button"
                size="sm"
                variant="outline"
                disabled={scanBusy}
                class="gap-1.5"
                aria-label="Скан"
                onclick={() => void onScan()}
              >
                <ScanSearchIcon class="size-4" />
              </Button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content portalDisabled to={dialogEl ?? undefined}>Сканировать включённые</Tooltip.Content>
        </Tooltip.Root>
        <Button
          type="button"
          size="sm"
          disabled={scanBusy}
          class="gap-1.5"
          onclick={() => void scanAndClose()}
        >
          <ScanSearchIcon class="size-4" />
          Скан и закрыть
        </Button>
      </footer>
    </div>
  </Tooltip.Provider>
</dialog>

{#if rulesPath}
  {@const target = entryFor(rulesPath)}
  {@const draft = target?.rules ?? {}}
  <dialog
    open
    class="fixed left-1/2 top-1/2 z-[60] w-[min(24rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-border bg-card p-4 shadow-xl backdrop:bg-black/40"
    aria-label="Правила папки"
    onclick={(e) => {
      if (e.currentTarget === e.target) rulesPath = null;
    }}
  >
    <form
      class="space-y-3"
      onsubmit={(e) => {
        e.preventDefault();
        const fd = new FormData(e.currentTarget);
        const maxDepthRaw = String(fd.get("max_depth") ?? "").trim();
        const maxNestRaw = String(fd.get("max_nesting") ?? "").trim();
        const ignoreRaw = String(fd.get("ignore") ?? "").trim();
        const rules: ScanRootRules = {
          max_depth: maxDepthRaw === "" ? null : Number(maxDepthRaw),
          max_nesting: maxNestRaw === "" ? null : Number(maxNestRaw),
          ignore: ignoreRaw
            ? ignoreRaw
                .split(/[\n,]/)
                .map((s) => s.trim())
                .filter(Boolean)
            : [],
        };
        const empty =
          rules.max_depth == null &&
          rules.max_nesting == null &&
          (!rules.ignore || rules.ignore.length === 0);
        void saveRules(rulesPath!, empty ? null : rules);
      }}
    >
      <h3 class="truncate text-sm font-medium" title={rulesPath}>Правила · {basename(rulesPath)}</h3>
      <p class="text-xs text-muted-foreground">
        Если папка внутри другого корня — её правила важнее для всех путей под ней.
      </p>
      <label class="block space-y-1 text-xs">
        <span class="text-muted-foreground">max_depth (пусто = глобальный)</span>
        <Input
          name="max_depth"
          type="number"
          min="0"
          max="12"
          value={draft.max_depth ?? ""}
          class="h-8 text-xs"
        />
      </label>
      <label class="block space-y-1 text-xs">
        <span class="text-muted-foreground">max_nesting</span>
        <Input
          name="max_nesting"
          type="number"
          min="0"
          max="6"
          value={draft.max_nesting ?? ""}
          class="h-8 text-xs"
        />
      </label>
      <label class="block space-y-1 text-xs">
        <span class="text-muted-foreground">ignore (запятая / строки)</span>
        <textarea
          name="ignore"
          rows="3"
          class="border-input w-full rounded-md border bg-transparent px-2 py-1.5 font-mono text-xs"
          value={(draft.ignore ?? []).join("\n")}
        ></textarea>
      </label>
      <div class="flex justify-end gap-2">
        <Button type="button" size="sm" variant="outline" onclick={() => (rulesPath = null)}
          >Отмена</Button
        >
        <Button
          type="button"
          size="sm"
          variant="ghost"
          onclick={() => void saveRules(rulesPath!, null)}>Сбросить</Button
        >
        <Button type="submit" size="sm">Сохранить</Button>
      </div>
    </form>
  </dialog>
{/if}
