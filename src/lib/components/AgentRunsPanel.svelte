<script lang="ts">
  import type { AgentRunRecord } from "$lib/types";
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
</script>

<section aria-labelledby="agent-runs-title">
  <h2 id="agent-runs-title">Ревью агентов</h2>
  <div class="row">
    <button type="button" disabled={runsBusy || actionBusy} onclick={onRefresh}
      >{runsBusy ? "Загрузка…" : "Обновить runs"}</button
    >
  </div>
  {#if runsError}
    <p class="error" role="alert">{runsError}</p>
  {/if}
  {#if mergeCommand}
    <div class="merge-box" role="status">
      <p>Принято. Слейте ветку вручную:</p>
      <pre class="merge-cmd">{mergeCommand}</pre>
    </div>
  {/if}
  {#if confirmDiscardId}
    <div class="confirmation" role="alertdialog" aria-labelledby="agent-discard-title">
      <p id="agent-discard-title">
        Отклонить run «{confirmDiscardId}» и удалить worktree? Ветка агента будет удалена.
      </p>
      <button type="button" disabled={actionBusy} onclick={discardConfirmed}>Да, отклонить</button>
      <button type="button" disabled={actionBusy} onclick={() => { confirmDiscardId = null; }}
        >Отмена</button
      >
    </div>
  {/if}
  {#if runs.length === 0}
    <p>Нет запусков агента. Появятся после старта из задачи.</p>
  {:else}
    <div class="table-scroll">
      <table>
        <thead>
          <tr>
            <th>Run</th>
            <th>Проект</th>
            <th>Раннер</th>
            <th>Статус</th>
            <th>База</th>
          </tr>
        </thead>
        <tbody>
          {#each runs as run (run.id)}
            <tr class:selected={run.id === selectedId}>
              <td>
                <button type="button" onclick={() => select(run.id)} aria-pressed={run.id === selectedId}
                  >{run.id}</button
                >
              </td>
              <td>{run.project}</td>
              <td>{agentRunnerLabel(run.runner)}</td>
              <td>{agentRunStatusLabel(run.status)}</td>
              <td>{run.base_ref}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  {#if selected}
    <article class="detail" aria-label="Детали run {selected.id}">
      <header class="detail-head">
        <h3>{selected.id}</h3>
        <p
          >{agentRunStatusLabel(selected.status)} · {agentRunnerLabel(selected.runner)} ·
          {selected.project_path}</p
        >
        <div class="row">
          {#if canAcceptAgentRun(selected.status)}
            <button type="button" disabled={actionBusy} onclick={acceptSelected}
              >Принять</button
            >
          {/if}
          {#if canDiscardAgentRun(selected.status)}
            <button
              type="button"
              disabled={actionBusy}
              onclick={() => {
                confirmDiscardId = selected.id;
              }}>Отклонить</button
            >
          {/if}
        </div>
      </header>
      {#if hasAgentRunReviewPayload(selected)}
        <h4>Diff</h4>
        <pre class="diff">{selected.diff ?? "(пусто)"}</pre>
        <h4>Лог тестов</h4>
        <pre class="log">{selected.tests?.log ?? "(нет лога)"}</pre>
        {#if selected.tests}
          <p class="meta">
            {#if selected.tests.skipped}
              тесты пропущены
            {:else}
              exit {selected.tests.exit_code ?? "—"} · {selected.tests.duration_ms} мс ·
              {selected.tests.command || "—"}
            {/if}
          </p>
        {/if}
      {:else}
        <p>Diff и лог ещё не собраны для этого run.</p>
      {/if}
    </article>
  {/if}
</section>

<style>
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 8px 0;
    flex-wrap: wrap;
  }
  .error {
    color: #b00020;
    overflow-wrap: anywhere;
  }
  .table-scroll {
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    padding: 4px 8px;
    border-bottom: 1px solid #ddd;
  }
  tr.selected {
    background: #f0f0ff;
  }
  .confirmation {
    background: #fff4e5;
    border: 1px solid #e0a800;
    border-radius: 6px;
    padding: 12px;
    margin: 8px 0;
  }
  .merge-box {
    background: #e8f5e9;
    border: 1px solid #81c784;
    border-radius: 6px;
    padding: 12px;
    margin: 8px 0;
  }
  .merge-cmd,
  .diff,
  .log {
    background: #0d0d12;
    color: #ddd;
    padding: 12px;
    overflow: auto;
    max-height: 280px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: 0.85rem;
  }
  .detail {
    margin-top: 16px;
    border-top: 1px solid #ddd;
    padding-top: 12px;
  }
  .detail-head h3 {
    margin: 0 0 4px;
  }
  .detail-head p {
    margin: 0 0 8px;
    color: #555;
    font-size: 0.9rem;
    overflow-wrap: anywhere;
  }
  .meta {
    color: #555;
    font-size: 0.85rem;
  }
</style>
