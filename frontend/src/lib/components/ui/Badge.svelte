<script lang="ts">
	import type { Snippet } from 'svelte';
	
	interface Props {
		variant?: 'filled' | 'outlined' | 'tonal';
		size?: 'small' | 'medium';
		color?: 'default' | 'profit' | 'loss' | 'warning' | 'info';
		children: Snippet;
		class?: string;
	}

	let {
		variant = 'tonal',
		size = 'small',
		color = 'default',
		children,
		class: className = ''
	}: Props = $props();

	const baseClasses = 'inline-flex items-center justify-center font-medium rounded-full transition-colors duration-fast';
	
	const sizeClasses = {
		small: 'px-2.5 py-1 text-label-sm gap-1',
		medium: 'px-3 py-1.5 text-label-md gap-1.5'
	};
	
	const variantColorClasses = {
		filled: {
			default: 'bg-primary text-on-primary',
			profit: 'bg-profit text-white',
			loss: 'bg-loss text-white',
			warning: 'bg-warning text-black',
			info: 'bg-info text-white'
		},
		tonal: {
			default: 'bg-primary-container text-on-primary-container',
			profit: 'bg-[var(--color-profit-container)] text-[var(--color-profit)]',
			loss: 'bg-[var(--color-loss-container)] text-[var(--color-loss)]',
			warning: 'bg-[var(--color-warning-container)] text-[var(--color-warning)]',
			info: 'bg-[var(--color-info-container)] text-[var(--color-info)]'
		},
		outlined: {
			default: 'border border-outline text-primary bg-transparent',
			profit: 'border border-profit text-profit bg-transparent',
			loss: 'border border-loss text-loss bg-transparent',
			warning: 'border border-warning text-warning bg-transparent',
			info: 'border border-info text-info bg-transparent'
		}
	};
	
	let badgeClasses = $derived(
		`${baseClasses} ${sizeClasses[size]} ${variantColorClasses[variant][color]} ${className}`
	);
</script>

<span class={badgeClasses}>
	{@render children()}
</span>
