<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";
  // Молекула: подпись и поле ввода. Состав: текст + atom Input.
  let { label = "Интервал, сек", value = "5", disabled = false }: { label?: string; value?: string; disabled?: boolean } = $props();
</script>
<div class="flex items-center justify-between gap-3 text-sm">
  <span>{label}</span>
  <Input class="w-28" {value} {disabled} aria-label={label} />
</div>
