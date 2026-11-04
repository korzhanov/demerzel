<script lang="ts">
  import type { AgentRunRecord, AgentRunner } from "$lib/types";
  import { invoke } from "@tauri-apps/api/core";
  import {
    agentRunnerLabel,
    agentRunStatusLabel,
    canAcceptAgentRun,
    canDiscardAgentRun,
    hasAgentRunReviewPayload,
  } from "$lib/agents";

  interface Props {
    runs: AgentRunRecord[];
    runsError: string;
    runsBusy: boolean;
    selectedId: string | null;
    mergeCommand: string;
    onRefresh: () => void;
    onSelect: (id: string) => void;
    onAccept: (id: string) => void | Promise<void>;
    onDiscard: (id: string) => void | Promise<void>;
  }

  let {
    runs,
    runsError,
    runsBusy,
    selectedId = $bindable<string | null>(null),
    mergeCommand = "",
    onRefresh,
    onSelect,
    onAccept,
    onDiscard,
  }: Props = $props();

  let actionBusy = $state(false);
  let confirmDiscardId = $state<string | null>(null);

  const selected = $derived(
    selectedId ? (runs.find((r) => r.id === selectedId) ?? null) : null,
  );

  function select(id: string) {
    selectedId = id;
    onSelect(id);
  }

  function runDuration(run: AgentRunRecord) {
    if (!run.started_at) return null;
    const started = Date.parse(run.started_at);
    const ended = run.finished_at ? Date.parse(run.finished_at) : Date.now();
    if (!Number.isFinite(started) || !Number.isFinite(ended)) return null;
    const totalSeconds = Math.max(0, Math.floor((ended - started) / 1000));
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    return hours ? `${hours} ч ${minutes} мин` : minutes ? `${minutes} мин ${seconds} сек` : `${seconds} сек`;
  }

  function tokenTotal(run: AgentRunRecord) {
    const total = (usage: AgentRunRecord["token_usage"]) => usage
      ? usage.input_tokens + usage.output_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens
      : 0;
    const tokens = total(run.token_usage) + total(run.review?.token_usage);
    return tokens > 0 ? tokens : null;
  }

  // Ревью другим агентом (#review): backend сохраняет его в запуске; здесь — запрос и последний ответ.
  let reviewBusy = $state(false);
  let reviewError = $state("");
  let reviewFresh = $state<Record<string, { runner: AgentRunner; text: string }>>({});

  async function requestReview() {
    if (!selected || reviewBusy) return;
    const id = selected.id;
    reviewBusy = true;
    reviewError = "";
    try {
      const rev = await invoke<{ runner: AgentRunner; text: string }>("request_agent_review", { runId: id, reviewer: null });
      reviewFresh = { ...reviewFresh, [id]: rev };
    } catch (e) {
      reviewError = String(e);
    } finally {
      reviewBusy = false;
    }
  }

  async function acceptSelected() {
    if (!selected || !canAcceptAgentRun(selected.status) || actionBusy) return;
    actionBusy = true;
    try {
      await onAccept(selected.id);
    } finally {
      actionBusy = false;
    }
  }

  async function discardConfirmed() {
    const id = confirmDiscardId;
    if (!id || actionBusy) return;
    confirmDiscardId = null;
    actionBusy = true;
    try {
      await onDiscard(id);
    } finally {
      actionBusy = false;
    }
  }
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
</script>

<Card.Root class="gap-3 rounded-lg py-3 shadow-none" aria-labelledby="agent-runs-title">
  <Card.Header class="px-4 pb-0">
    <Card.Title id="agent-runs-title" class="text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">Ревью агентов</Card.Title>
  </Card.Header>
  <Card.Content class="space-y-3">
    <Button variant="outline" disabled={runsBusy || actionBusy} onclick={onRefresh}>
      {runsBusy ? "Загрузка…" : "Обновить runs"}
    </Button>
    {#if runsError}
      <p class="break-words text-sm text-destructive" role="alert">{runsError}</p>
    {/if}
    {#if mergeCommand}
      <div class="space-y-1 rounded-md border border-primary p-3 text-sm" role="status">
        <p>Принято. Слейте ветку вручную:</p>
        <pre class="overflow-x-auto font-mono text-xs">{mergeCommand}</pre>
      </div>
    {/if}
    {#if confirmDiscardId}
      <div class="space-y-2 rounded-md border border-destructive p-3 text-sm" role="alertdialog" aria-labelledby="agent-discard-title">
        <p id="agent-discard-title">Отклонить run «{confirmDiscardId}» и удалить worktree? Ветка агента будет удалена.</p>
        <div class="flex gap-2">
          <Button variant="destructive" disabled={actionBusy} onclick={discardConfirmed}>Да, отклонить</Button>
          <Button variant="outline" disabled={actionBusy} onclick={() => { confirmDiscardId = null; }}>Отмена</Button>
        </div>
      </div>
    {/if}
    {#if runs.length === 0}
      <p class="text-sm text-muted-foreground">Нет запусков агента. Появятся после старта из задачи.</p>
    {:else}
      <Table.Root>
        <Table.Header>
          <Table.Row>
            <Table.Head>Run</Table.Head>
            <Table.Head>Проект</Table.Head>
            <Table.Head>Раннер</Table.Head>
            <Table.Head>Статус</Table.Head>
            <Table.Head>База</Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each runs as run (run.id)}
            <Table.Row data-state={run.id === selectedId ? "selected" : undefined}>
              <Table.Cell>
                <Button variant="link" class="h-auto p-0" onclick={() => select(run.id)} aria-pressed={run.id === selectedId}>{run.id}</Button>
              </Table.Cell>
              <Table.Cell>{run.project}</Table.Cell>
              <Table.Cell>{agentRunnerLabel(run.runner)}</Table.Cell>
              <Table.Cell><Badge variant="secondary">{agentRunStatusLabel(run.status)}</Badge></Table.Cell>
              <Table.Cell class="font-mono text-xs">{run.base_ref}</Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    {/if}

    {#if selected}
      <article class="space-y-2 rounded-lg border p-3" aria-label="Детали run {selected.id}">
        <header class="space-y-1">
          <h3 class="font-medium">{selected.id}</h3>
          <p class="text-xs text-muted-foreground">
            {agentRunStatusLabel(selected.status)} · {agentRunnerLabel(selected.runner)} · {selected.project_path}
          </p>
          <p class="text-xs text-muted-foreground">
            {#if runDuration(selected)}Время: {runDuration(selected)}{/if}
            {#if tokenTotal(selected) !== null}{runDuration(selected) ? " · " : ""}Токены: {tokenTotal(selected)!.toLocaleString("ru-RU")}{/if}
            {#if selected.finished_at && tokenTotal(selected) === null}{runDuration(selected) ? " · " : ""}Токены: метрика недоступна{/if}
          </p>
          <div class="flex gap-2">
            {#if canAcceptAgentRun(selected.status)}
              <Button disabled={actionBusy} onclick={acceptSelected}>Принять</Button>
            {/if}
            {#if canDiscardAgentRun(selected.status)}
              <Button variant="outline" disabled={actionBusy} onclick={() => { confirmDiscardId = selected.id; }}>Отклонить</Button>
            {/if}
          </div>
        </header>
        {#if hasAgentRunReviewPayload(selected)}
          <div class="flex items-center gap-2">
            <Button variant="outline" size="sm" disabled={reviewBusy || !selected.diff} onclick={requestReview}>
              {reviewBusy ? "Ревью идёт…" : "Ревью другим агентом"}
            </Button>
            {#if reviewError}<span class="text-xs text-destructive">{reviewError}</span>{/if}
          </div>
          {#if reviewFresh[selected.id] ?? selected.review}
            {@const rev = reviewFresh[selected.id] ?? selected.review}
            <h4 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Ревью ({agentRunnerLabel(rev!.runner)})</h4>
            <pre class="max-h-72 overflow-auto whitespace-pre-wrap rounded-md bg-background p-3 text-xs">{rev!.text}</pre>
          {/if}
          <h4 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Diff</h4>
          <pre class="max-h-72 overflow-auto rounded-md bg-background p-3 font-mono text-xs">{selected.diff ?? "(пусто)"}</pre>
          <h4 class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Лог тестов</h4>
          <pre class="max-h-72 overflow-auto rounded-md bg-background p-3 font-mono text-xs">{selected.tests?.log ?? "(нет лога)"}</pre>
          {#if selected.tests}
            <p class="text-xs text-muted-foreground">
              {#if selected.tests.skipped}
                тесты пропущены
              {:else}
                exit {selected.tests.exit_code ?? "—"} · {selected.tests.duration_ms} мс · {selected.tests.command || "—"}
              {/if}
            </p>
          {/if}
        {:else}
          <p class="text-sm text-muted-foreground">Diff и лог ещё не собраны для этого run.</p>
        {/if}
      </article>
    {/if}
  </Card.Content>
</Card.Root>
