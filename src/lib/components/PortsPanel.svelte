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
  import { Button } from "$lib/components/ui/button/index.js";
  import * as Card from "$lib/components/ui/card/index.js";
  import * as Table from "$lib/components/ui/table/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
</script>

<Card.Root aria-labelledby="ports-title">
  <Card.Header>
    <Card.Title id="ports-title" class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Порты</Card.Title>
  </Card.Header>
  <Card.Content class="space-y-3">
    <div class="flex flex-wrap items-center gap-3">
      <Button variant="outline" disabled={portsBusy || stopping} onclick={onRefresh}>Обновить порты</Button>
      <label class="flex items-center gap-2 text-sm text-muted-foreground">
        Интервал опроса, секунд
        <Input class="w-20" type="number" min="1" max="60" bind:value={portInterval} />
      </label>
    </div>
    {#if portsError}
      <p class="break-words text-sm text-destructive" role="alert">{portsError}</p>
    {/if}
    {#if confirmPort}
      <div class="space-y-2 rounded-md border border-destructive p-3 text-sm" role="alertdialog" aria-labelledby="ports-confirm-title">
        <p id="ports-confirm-title">
          Остановить управляемый процесс {confirmPort.command} (PID {confirmPort.pid}) проекта {confirmPort.project}?
          Будет остановлена вся группа процесса.
        </p>
        <div class="flex gap-2">
          <Button variant="destructive" disabled={stopping} onclick={confirmStop}>Да, остановить</Button>
          <Button variant="outline" disabled={stopping} onclick={() => { confirmPort = null; }}>Отмена</Button>
        </div>
      </div>
    {/if}
    {#if ports.length === 0}
      <p class="text-sm text-muted-foreground">Слушающих портов не найдено.</p>
    {:else}
      <Table.Root>
        <Table.Header>
          <Table.Row>
            <Table.Head>Порт</Table.Head>
            <Table.Head>Проект</Table.Head>
            <Table.Head>Процесс</Table.Head>
            <Table.Head>PID</Table.Head>
            <Table.Head>Статус</Table.Head>
            <Table.Head>Действие</Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each ports as port (`${port.pid}:${port.port}`)}
            <Table.Row>
              <Table.Cell class="font-mono">{port.port}</Table.Cell>
              <Table.Cell>{port.project ?? "—"}</Table.Cell>
              <Table.Cell class="font-mono text-xs">{port.command}</Table.Cell>
              <Table.Cell>{port.pid}</Table.Cell>
              <Table.Cell>
                <Badge variant={port.status === "conflict" ? "destructive" : port.status === "foreign" ? "outline" : "secondary"}>
                  {portStatusLabel(port.status)}
                </Badge>
              </Table.Cell>
              <Table.Cell>
                {#if canStopManagedPort(port)}
                  <Button size="sm" variant="outline" disabled={stopping} onclick={() => { confirmPort = port; }}>Остановить…</Button>
                {:else}
                  <span class="text-xs text-muted-foreground">Только просмотр</span>
                {/if}
              </Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    {/if}
  </Card.Content>
</Card.Root>
