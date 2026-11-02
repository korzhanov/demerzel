<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  let { label = "Кнопка", variant = "default", size = "default", disabled = false }: {
    label?: string; variant?: "default" | "outline" | "secondary" | "ghost" | "destructive" | "link"; size?: "default" | "sm" | "lg"; disabled?: boolean;
  } = $props();
</script>
<Button {variant} {size} {disabled}>{label}</Button>
