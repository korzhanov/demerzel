<script lang="ts">
  // Песочница компонентов /test: atomic design, контролы генерируются из схемы, справа JSON props.
  import { stories, LEVELS, type Control, type Story } from "$lib/sandbox/registry";

  let selectedId = $state(stories[0].id);
  let current = $derived(stories.find((s) => s.id === selectedId) ?? stories[0]);
  // Правки props хранятся отдельно для каждой истории, чтобы переключение не теряло изменения.
  let overrides = $state<Record<string, Record<string, unknown>>>({});
  let props = $derived({ ...current.props, ...(overrides[current.id] ?? {}) });
  let Cmp = $derived(current.component);

  function setProp(story: Story, key: string, value: unknown) {
    overrides = { ...overrides, [story.id]: { ...(overrides[story.id] ?? {}), [key]: value } };
  }

  function readControl(c: Control, el: HTMLInputElement | HTMLSelectElement): unknown {
    if (c.type === "boolean") return (el as HTMLInputElement).checked;
    if (c.type === "number") return Number(el.value);
    return el.value;
  }

  function jsonProps(p: Record<string, unknown>): string {
    return JSON.stringify(p, (_k, v) => (typeof v === "function" ? "[fn]" : v), 2);
  }
</script>

<div class="flex min-h-screen bg-background text-foreground">
  <nav class="w-56 shrink-0 space-y-4 overflow-y-auto border-r border-border p-3 text-sm">
    <h1 class="font-medium">Песочница</h1>
    {#each LEVELS as level (level)}
      <div>
        <p class="mb-1 text-[11px] uppercase tracking-[0.08em] text-muted-foreground">{level}</p>
        <ul class="space-y-0.5">
          {#each stories.filter((s) => s.level === level) as s (s.id)}
            <li>
              <button
                class="w-full rounded px-2 py-1 text-left {s.id === selectedId
                  ? 'bg-accent font-medium'
                  : 'hover:bg-accent/50'}"
                onclick={() => (selectedId = s.id)}>{s.title}</button
              >
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  </nav>

  <main class="grid flex-1 gap-4 p-4 lg:grid-cols-[1fr_20rem]">
    <section class="min-w-0 space-y-3">
      <h2 class="text-lg font-medium">
        {current.title} <span class="text-xs text-muted-foreground">· {current.level}</span>
      </h2>
      <div class="overflow-auto rounded-lg border border-border bg-card p-6">
        <Cmp {...props} />
      </div>
    </section>

    <aside class="space-y-4">
      <div class="space-y-2 rounded-lg border border-border p-3">
        <p class="text-[11px] uppercase tracking-[0.08em] text-muted-foreground">Контролы</p>
        {#each current.controls as c (c.key)}
          <label class="flex items-center justify-between gap-2 text-sm">
            <span>{c.label}</span>
            {#if c.type === "boolean"}
              <input
                type="checkbox"
                checked={!!props[c.key]}
                onchange={(e) => setProp(current, c.key, readControl(c, e.currentTarget))}
              />
            {:else if c.type === "select"}
              <select
                class="rounded border border-input bg-background px-1 text-xs"
                value={String(props[c.key])}
                onchange={(e) => setProp(current, c.key, readControl(c, e.currentTarget))}
              >
                {#each c.options as opt (opt)}<option value={opt}>{opt}</option>{/each}
              </select>
            {:else}
              <input
                class="w-32 rounded border border-input bg-background px-1 text-xs"
                type={c.type === "number" ? "number" : "text"}
                min={c.type === "number" ? c.min : undefined}
                max={c.type === "number" ? c.max : undefined}
                value={String(props[c.key] ?? "")}
                onchange={(e) => setProp(current, c.key, readControl(c, e.currentTarget))}
              />
            {/if}
          </label>
        {/each}
      </div>

      <div class="space-y-1">
        <p class="text-[11px] uppercase tracking-[0.08em] text-muted-foreground">Props (JSON)</p>
        <pre class="max-h-80 overflow-auto rounded-md border border-border bg-background p-2 text-xs">{jsonProps(props)}</pre>
      </div>
    </aside>
  </main>
</div>
