<script lang="ts">
	import { Checkbox as CheckboxPrimitive } from "bits-ui";
	import CheckIcon from "@lucide/svelte/icons/check";
	import MinusIcon from "@lucide/svelte/icons/minus";
	import { cn, type WithoutChildrenOrChild } from "$lib/utils.js";
	import type { ComponentProps } from "svelte";

	let {
		ref = $bindable(null),
		checked = $bindable(false),
		indeterminate = $bindable(false),
		class: className,
		...restProps
	}: WithoutChildrenOrChild<ComponentProps<typeof CheckboxPrimitive.Root>> = $props();
</script>

<CheckboxPrimitive.Root
	bind:ref
	bind:checked
	bind:indeterminate
	data-slot="checkbox"
	class={cn(
		"peer size-4 shrink-0 rounded-[4px] border-2 border-foreground/55 bg-background text-transparent shadow-xs outline-none transition-[color,box-shadow,background-color,border-color]",
		"hover:border-foreground/80",
		"focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50",
		"disabled:cursor-not-allowed disabled:opacity-50",
		"aria-invalid:border-destructive aria-invalid:ring-destructive/30",
		/* checked / mixed — всегда поверх темы (не перекрывать light:/dark: фоном) */
		"data-[state=checked]:border-primary data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground",
		"data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary data-[state=indeterminate]:text-primary-foreground",
		"aria-[checked=true]:border-primary aria-[checked=true]:bg-primary aria-[checked=true]:text-primary-foreground",
		"aria-[checked=mixed]:border-primary aria-[checked=mixed]:bg-primary aria-[checked=mixed]:text-primary-foreground",
		className
	)}
	{...restProps}
>
	{#snippet children({ checked, indeterminate })}
		<div
			data-slot="checkbox-indicator"
			class="flex size-full items-center justify-center text-current transition-none"
		>
			{#if indeterminate}
				<MinusIcon class="size-3 stroke-[3]" aria-hidden="true" />
			{:else if checked}
				<CheckIcon class="size-3 stroke-[3]" aria-hidden="true" />
			{/if}
		</div>
	{/snippet}
</CheckboxPrimitive.Root>
