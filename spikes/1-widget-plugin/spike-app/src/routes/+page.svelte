<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let failing = $state<number | null>(null);
  let error = $state("");

  async function runTick() {
    error = "";
    try {
      failing = await invoke<number>("tick");
    } catch (e) {
      error = String(e);
    }
  }
</script>

<main class="wrap">
  <h1>Demerzel spike</h1>
  <p>Виджет обновляется из Rust каждые 10 с. Кнопка — ручной tick.</p>
  <button type="button" onclick={runTick}>tick</button>
  {#if failing !== null}
    <p>failing = {failing}</p>
  {/if}
  {#if error}
    <p class="err">{error}</p>
  {/if}
</main>

<style>
  .wrap {
    margin: 0;
    padding: 2rem;
    font-family: ui-sans-serif, system-ui, sans-serif;
  }
  .err {
    color: #e53935;
  }
  button {
    padding: 0.5rem 1rem;
    cursor: pointer;
  }
</style>
