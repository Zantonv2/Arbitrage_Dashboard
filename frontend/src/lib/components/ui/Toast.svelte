<script lang="ts">
	import { toastStore, type Toast as ToastType } from '$lib/stores/toast.svelte';
	
	function getIcon(type: ToastType['type']) {
		switch (type) {
			case 'success':
				return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>`;
			case 'error':
				return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>`;
			case 'warning':
				return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>`;
			case 'info':
				return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>`;
		}
	}
	
	function getTypeClass(type: ToastType['type']) {
		switch (type) {
			case 'success':
				return 'toast-success';
			case 'error':
				return 'toast-error';
			case 'warning':
				return 'toast-warning';
			case 'info':
				return 'toast-info';
		}
	}
</script>

<div class="toast-container" role="alert" aria-live="polite">
	{#each toastStore.toasts as toast (toast.id)}
		<div class="toast {getTypeClass(toast.type)}">
			<div class="toast-icon" aria-hidden="true">
				{@html getIcon(toast.type)}
			</div>
			<div class="toast-content">
				<div class="toast-title">{toast.title}</div>
				{#if toast.message}
					<div class="toast-message">{toast.message}</div>
				{/if}
			</div>
			<button 
				class="toast-close" 
				onclick={() => toastStore.remove(toast.id)}
				aria-label="Dismiss notification"
			>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</button>
		</div>
	{/each}
</div>

<style>
	.toast-container {
		position: fixed;
		bottom: var(--space-4);
		right: var(--space-4);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		z-index: 1000;
		max-width: 400px;
	}

	.toast {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--shape-large);
		background: var(--md-sys-color-surface-container-high);
		border: 1px solid var(--glass-border);
		box-shadow: var(--glass-elevation-3);
		animation: slide-in 0.3s ease-out;
	}

	@keyframes slide-in {
		from {
			transform: translateX(100%);
			opacity: 0;
		}
		to {
			transform: translateX(0);
			opacity: 1;
		}
	}

	.toast-success {
		border-left: 4px solid var(--color-profit);
	}

	.toast-error {
		border-left: 4px solid var(--color-loss);
	}

	.toast-warning {
		border-left: 4px solid #f59e0b;
	}

	.toast-info {
		border-left: 4px solid var(--md-sys-color-primary);
	}

	.toast-icon {
		flex-shrink: 0;
		width: 20px;
		height: 20px;
	}

	.toast-success .toast-icon {
		color: var(--color-profit);
	}

	.toast-error .toast-icon {
		color: var(--color-loss);
	}

	.toast-warning .toast-icon {
		color: #f59e0b;
	}

	.toast-info .toast-icon {
		color: var(--md-sys-color-primary);
	}

	.toast-content {
		flex: 1;
		min-width: 0;
	}

	.toast-title {
		font: var(--typography-label-medium);
		font-weight: 600;
		color: var(--md-sys-color-on-surface);
	}

	.toast-message {
		font: var(--typography-body-small);
		color: var(--md-sys-color-on-surface-variant);
		margin-top: var(--space-1);
		word-wrap: break-word;
	}

	.toast-close {
		flex-shrink: 0;
		width: 20px;
		height: 20px;
		padding: 0;
		border: none;
		background: transparent;
		color: var(--md-sys-color-on-surface-variant);
		cursor: pointer;
		opacity: 0.7;
		transition: opacity var(--duration-short) var(--ease-standard);
	}

	.toast-close:hover {
		opacity: 1;
	}

	.toast-close svg {
		width: 100%;
		height: 100%;
	}
</style>
