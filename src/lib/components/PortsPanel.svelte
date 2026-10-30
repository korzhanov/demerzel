<script lang="ts">
  import type { PortRow } from "$lib/types";
  import { canStopManagedPort, portStatusLabel } from "$lib/ports";

  interface Props {
    ports: PortRow[];
    portsError: string;
    portsBusy: boolean;
    portInterval: number;
    onRefresh: () => void;
    onStopManaged: (managedId: number) => Promise<void>;
  }

  let {
    ports,
    portsError,
    portsBusy,
    portInterval = $bindable(5),
    onRefresh,
    onStopManaged,
  }: Props = $props();

  let confirmPort = $state<PortRow | null>(null);
  let stopping = $state(false);

  async function confirmStop() {
    const row = confirmPort;
    if (!row || !canStopManagedPort(row) || row.managed_id === null) return;
    confirmPort = null;
    stopping = true;
    try {
      await onStopManaged(row.managed_id);
    } finally {
      stopping = false;
    }
  }
</script>

<section aria-labelledby="ports-title">
  <h2 id="ports-title">Порты</h2>
  <div class="row">
    <button type="button" disabled={portsBusy || stopping} onclick={onRefresh}>Обновить порты</button>
    <label
      >Интервал опроса, секунд
      <input type="number" min="1" max="60" bind:value={portInterval} />
    </label>
  </div>
  {#if portsError}
    <p class="error" role="alert">{portsError}</p>
  {/if}
  {#if confirmPort}
    <div class="confirmation" role="alertdialog" aria-labelledby="ports-confirm-title">
      <p id="ports-confirm-title">
        Остановить управляемый процесс {confirmPort.command} (PID {confirmPort.pid}) проекта {confirmPort.project}?
        Будет остановлена вся группа процесса.
      </p>
      <button type="button" disabled={stopping} onclick={confirmStop}>Да, остановить</button>
      <button type="button" disabled={stopping} onclick={() => { confirmPort = null; }}>Отмена</button>
    </div>
  {/if}
  {#if ports.length === 0}
    <p>Слушающих портов не найдено.</p>
  {:else}
    <div class="table-scroll">
      <table>
        <thead>
          <tr>
            <th>Порт</th>
            <th>Проект</th>
            <th>Процесс</th>
            <th>PID</th>
            <th>Статус</th>
            <th>Действие</th>
          </tr>
        </thead>
        <tbody>
          {#each ports as port (`${port.pid}:${port.port}`)}
            <tr class:conflict={port.status === "conflict"} class:foreign={port.status === "foreign"}>
              <td>{port.port}</td>
              <td>{port.project ?? "—"}</td>
              <td>{port.command}</td>
              <td>{port.pid}</td>
              <td>{portStatusLabel(port.status)}</td>
              <td>
                {#if canStopManagedPort(port)}
                  <button
                    type="button"
                    disabled={stopping}
                    onclick={() => {
                      confirmPort = port;
                    }}>Остановить…</button
                  >
                {:else}
                  Только просмотр
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
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
  label {
    flex: 1;
    min-width: 240px;
  }
  input {
    width: 100%;
    font: inherit;
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
  .error {
    color: #b00020;
    overflow-wrap: anywhere;
  }
  .table-scroll {
    overflow-x: auto;
  }
  .conflict {
    background: #ffebee;
  }
  .foreign {
    background: #fff8e1;
  }
  .confirmation {
    padding: 12px;
    border: 1px solid #b00020;
    margin-bottom: 12px;
  }
  .confirmation button {
    margin-right: 8px;
  }
</style>
