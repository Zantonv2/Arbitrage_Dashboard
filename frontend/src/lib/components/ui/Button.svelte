<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'filled' | 'elevated' | 'outlined' | 'text' | 'tonal';
		size?: 'small' | 'medium' | 'large';
		disabled?: boolean;
		type?: 'button' | 'submit' | 'reset';
		onclick?: (e: MouseEvent) => void;
		children: Snippet;
		class?: string;
	}

	let {
		variant = 'filled',
		size = 'medium',
		disabled = false,
		type = 'button',
		onclick,
		children,
		class: className = ''
	}: Props = $props();

	const baseClasses = 'inline-flex items-center justify-center gap-2 font-medium transition-colors duration-fast ease-standard relative overflow-hidden whitespace-nowrap';
	
	const variantClasses = {
		filled: 'bg-[var(--md-sys-color-primary)] text-[var(--md-sys-color-on-primary)] hover:shadow-[var(--elevation-1)] active:shadow-[var(--elevation-0)]',
		elevated: 'glass glass-elevation-1 text-[var(--md-sys-color-primary)] hover:glass-elevation-2 active:glass-elevation-1',
		outlined: 'bg-transparent border border-[var(--md-sys-color-outline)] text-[var(--md-sys-color-primary)] hover:bg-[var(--md-sys-color-surface-container-high)]',
		text: 'bg-transparent text-[var(--md-sys-color-primary)] hover:bg-[var(--md-sys-color-surface-container-high)]',
		tonal: 'bg-[var(--md-sys-color-secondary-container)] text-[var(--md-sys-color-on-secondary-container)] hover:shadow-[var(--elevation-1)]'
	};
	
	const sizeClasses = {
		small: 'h-8 px-4 text-label-md rounded-lg',
		medium: 'h-10 px-6 text-label-lg rounded-full',
		large: 'h-12 px-8 text-label-lg rounded-full'
	};
	
	const disabledClasses = 'opacity-[var(--state-disabled-opacity)] cursor-not-allowed pointer-events-none';
	
	let buttonClasses = $derived(
		`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${disabled ? disabledClasses : ''} ${className}`
	);

	// Svelte 5 reactive state for ripples
	let ripples = $state<Array<{id: number; x: number; y: number}>>([]);
	
	function handleClick(e: MouseEvent) {
		// Create ripple effect using Svelte 5 reactive state
		const button = e.currentTarget as HTMLButtonElement;
		const rect = button.getBoundingClientRect();
		const x = e.clientX - rect.left;
		const y = e.clientY - rect.top;
		
		const id = Date.now();
		ripples = [...ripples, { id, x, y }];
		
		// Remove ripple after animation completes
		setTimeout(() => {
			ripples = ripples.filter(r => r.id !== id);
		}, 600);
		
		onclick?.(e);
	}
</script>

<button
	{type}
	class={buttonClasses}
	disabled={disabled}
	onclick={handleClick}
	aria-hidden="true"
>
	{@render children()}
	{#each ripples as ripple (ripple.id)}
		<span
			class="ripple"
			style="left: {ripple.x}px; top: {ripple.y}px;"
		></span>
	{/each}
</button>

<style>
	.ripple {
		position: absolute;
		border-radius: 50%;
		background: currentColor;
		opacity: 0.2;
		transform: scale(0);
		pointer-events: none;
		width: 100px;
		height: 100px;
		margin-left: -50px;
		margin-top: -50px;
		animation: ripple 0.6s ease-out forwards;
	}

	@keyframes ripple {
		to {
			transform: scale(4);
			opacity: 0;
		}
	}
</style>
