<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'elevated' | 'filled' | 'outlined';
		padding?: 'none' | 'small' | 'medium' | 'large';
		interactive?: boolean;
		children: Snippet;
		class?: string;
		onclick?: (e: MouseEvent) => void;
	}

	let {
		variant = 'elevated',
		padding = 'medium',
		interactive = false,
		children,
		class: className = '',
		onclick
	}: Props = $props();

	const baseClasses = 'rounded-xl transition-all duration-fast ease-standard overflow-hidden h-full';
	
	const variantClasses = {
		elevated: 'glass glass-elevation-1',
		filled: 'bg-[var(--md-sys-color-surface-container-low)] shadow-[var(--elevation-1)]',
		outlined: 'bg-transparent border border-[var(--md-sys-color-outline-variant)]'
	};
	
	const paddingClasses = {
		none: '',
		small: 'p-3',
		medium: 'p-5',
		large: 'p-6'
	};
	
	const interactiveClasses = interactive 
		? 'cursor-pointer hover:-translate-y-0.5 hover:shadow-[var(--elevation-2)] active:translate-y-0 active:shadow-[var(--elevation-1)]' 
		: '';
	
	let cardClasses = $derived(
		`${baseClasses} ${variantClasses[variant]} ${paddingClasses[padding]} ${interactiveClasses} ${className}`
	);
</script>

{#if interactive}
	<div
		class={cardClasses}
		onclick={onclick}
		role="button"
		tabindex="0"
		onkeydown={(e) => e.key === 'Enter' && onclick?.(e as unknown as MouseEvent)}
	>
		{@render children()}
	</div>
{:else}
	<div class={cardClasses}>
		{@render children()}
	</div>
{/if}
