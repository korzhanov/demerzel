<script lang="ts">
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
  // Молекула: флажок с подписью. Состав: atom Checkbox + текст.
  let { label = "Сканировать папку", checked = false, disabled = false }: { label?: string; checked?: boolean; disabled?: boolean } = $props();
  let state = $state(false);
  $effect(() => { state = checked; });
</script>
<label class="flex cursor-pointer items-center gap-2 text-sm">
  <Checkbox bind:checked={state} {disabled} aria-label={label} />
  <span>{label}</span>
</label>
