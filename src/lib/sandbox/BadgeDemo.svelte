<script lang="ts">
  import { Badge } from "$lib/components/ui/badge/index.js";
  let { label = "статус", variant = "default" }: { label?: string; variant?: "default" | "secondary" | "destructive" | "outline" } = $props();
</script>
<Badge {variant}>{label}</Badge>
