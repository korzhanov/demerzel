<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";
	import type { ComponentProps } from "svelte";

	let {
		ref = $bindable(null),
		class: className,
		sideOffset = 4,
		children,
		/** Куда порталить (по умолчанию body). Для `<dialog>` передай сам dialog — иначе top-layer его перекрывает. */
		to = undefined as Element | string | undefined,
		/** Не порталить: контент остаётся в том же top-layer, что и dialog. */
		portalDisabled = false,
		...restProps
	}: ComponentProps<typeof TooltipPrimitive.Content> & {
		to?: Element | string;
		portalDisabled?: boolean;
	} = $props();
</script>

<TooltipPrimitive.Portal {to} disabled={portalDisabled}>
	<TooltipPrimitive.Content
		bind:ref
		data-slot="tooltip-content"
		{sideOffset}
		class={cn(
			"bg-primary text-primary-foreground animate-in fade-in-0 zoom-in-95 z-[100] w-fit rounded-md px-3 py-1.5 text-xs text-balance",
			className
		)}
		{...restProps}
	>
		{@render children?.()}
		<TooltipPrimitive.Arrow
			class="bg-primary fill-primary z-[100] size-2.5 translate-y-[calc(-50%-2px)] rotate-45 rounded-[2px]"
		/>
	</TooltipPrimitive.Content>
</TooltipPrimitive.Portal>
