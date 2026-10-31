<script lang="ts">
	import { Switch as SwitchPrimitive } from "bits-ui";
	import { cn, type WithoutChildrenOrChild } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		checked = $bindable(false),
		size = "default",
		...restProps
	}: WithoutChildrenOrChild<SwitchPrimitive.RootProps> & {
		size?: "sm" | "default";
	} = $props();
</script>

<SwitchPrimitive.Root
	bind:ref
	bind:checked
	data-slot="switch"
	data-size={size}
	class={cn(
		"peer group/switch relative inline-flex shrink-0 items-center rounded-full border-2 outline-none transition-[color,background-color,border-color,box-shadow]",
		"focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50",
		"data-disabled:cursor-not-allowed data-disabled:opacity-50",
		"data-[size=default]:h-5 data-[size=default]:w-9 data-[size=sm]:h-4 data-[size=sm]:w-7",
		"data-[state=unchecked]:border-foreground/40 data-[state=unchecked]:bg-muted",
		"data-[state=checked]:border-primary data-[state=checked]:bg-primary",
		"aria-[checked=true]:border-primary aria-[checked=true]:bg-primary",
		"hover:data-[state=unchecked]:border-foreground/60",
		className,
	)}
	{...restProps}
>
	<SwitchPrimitive.Thumb
		data-slot="switch-thumb"
		class={cn(
			"pointer-events-none block rounded-full bg-background shadow-sm ring-0 transition-transform",
			"group-data-[size=default]/switch:size-4 group-data-[size=sm]/switch:size-3",
			"data-[state=unchecked]:translate-x-0.5",
			"data-[state=checked]:translate-x-[18px]",
			"group-data-[size=sm]/switch:data-[state=checked]:translate-x-[14px]",
		)}
	/>
</SwitchPrimitive.Root>
