<script lang="ts">
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import InfoIcon from "@lucide/svelte/icons/info";

  let {
    text,
    label = "Пояснение",
    portalDisabled = false,
  }: { text: string; label?: string; portalDisabled?: boolean } = $props();
</script>

<Tooltip.Provider delayDuration={200}>
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <button
          {...props}
          type="button"
          aria-label={label}
          class="inline-flex size-4 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        >
          <InfoIcon class="size-3.5" aria-hidden="true" />
        </button>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content side="bottom" align="start" {portalDisabled} class="max-w-xs text-left leading-snug">
      {text}
    </Tooltip.Content>
  </Tooltip.Root>
</Tooltip.Provider>
