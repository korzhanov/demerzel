<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";
	import type { ComponentProps } from "svelte";

	let {
		ref = $bindable(null),
		class: className,
		sideOffset = 4,
		children,
		/** Куда порталить (по умолчанию body). */
		to = undefined as Element | string | undefined,
		/**
		 * Не порталить — нужно внутри `<dialog>` (top-layer).
		 * Родитель dialog не должен иметь transform — иначе fixed съезжает.
		 */
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
		collisionPadding={8}
		class={cn(
			"bg-foreground text-background animate-in fade-in-0 zoom-in-95 z-[200] w-fit max-w-[min(20rem,calc(100vw-2rem))] rounded-md px-3 py-1.5 text-xs text-balance shadow-md",
			className
		)}
		{...restProps}
	>
		{@render children?.()}
		<!-- bits-ui рисует SVG-треугольник с currentColor; поворот/размер на обёртке ломают его. Цвет — через text-*. -->
		<TooltipPrimitive.Arrow class="text-foreground" width={10} height={5} />
	</TooltipPrimitive.Content>
</TooltipPrimitive.Portal>
