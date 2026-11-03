<script lang="ts">
  let { label = "Запускаем Demerzel…" }: { label?: string } = $props();
</script>

<div
  class="fixed inset-0 z-[100] flex flex-col items-center justify-center gap-5 bg-background/95 text-foreground backdrop-blur-sm"
  role="status"
  aria-live="polite"
>
  <img
    src="/mockups/icon.jpg"
    alt=""
    class="size-24 rounded-2xl object-cover shadow-[0_12px_48px_rgba(224,168,58,0.22)] ring-1 ring-primary/40"
  />
  <div class="text-center">
    <p class="text-lg font-semibold tracking-wide">DEMERZEL</p>
    <p class="mt-1 text-sm text-muted-foreground">{label}</p>
  </div>
  <div class="h-1 w-40 overflow-hidden rounded-full bg-secondary" aria-hidden="true">
    <div class="splash-progress h-full w-1/2 rounded-full bg-primary"></div>
  </div>
</div>

<style>
  .splash-progress {
    animation: loading 1.2s ease-in-out infinite;
  }

  @keyframes loading {
    from { transform: translateX(-110%); }
    to { transform: translateX(210%); }
  }
</style>
