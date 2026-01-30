<script lang="ts">
	import '../app.css';
	import { WebSocketClient } from '$lib/websocket/client.svelte';
	import { apiClient } from '$lib/api/client';
	import { signalsStore } from '$lib/stores/signals.svelte';
	import { exchangesStore } from '$lib/stores/exchanges.svelte';
	import { systemStore } from '$lib/stores/system.svelte';
	import { TradeSignalSchema, ExchangeStatusSchema, SystemStatusSchema } from '$lib/schemas';
	import { onMount } from 'svelte';
	import Sidebar from '$lib/components/ui/Sidebar.svelte';

	let wsClient: WebSocketClient;
	let darkMode = $state(false);
	let isLoading = $state(true);
	let sidebarExpanded = $state(true);

	onMount(async () => {
		// Initialize dark mode from localStorage
		const savedTheme = localStorage.getItem('theme');
		darkMode = savedTheme === 'dark' || (!savedTheme && window.matchMedia('(prefers-color-scheme: dark)').matches);
		document.documentElement.classList.toggle('dark', darkMode);

		// Fetch initial data from API
		try {
			const [signals, exchanges] = await Promise.all([
				apiClient.getSignals({ limit: 50 }),
				apiClient.getExchangeStatus()
			]);

			// Populate stores with initial data
			signals.forEach(signal => signalsStore.addSignal(signal));
			exchanges.forEach(exchange => exchangesStore.updateExchange(exchange));
		} catch (error) {
			console.error('Failed to fetch initial data:', error);
		} finally {
			isLoading = false;
		}

		// Initialize WebSocket for real-time updates
		wsClient = new WebSocketClient('ws://localhost:8080/ws');
		wsClient.connect();

		// Handle WebSocket messages
		const unsubscribe = wsClient.onMessage((message) => {
			try {
				switch (message.type) {
					case 'signal_update':
						const signal = TradeSignalSchema.parse(message.data);
						signalsStore.addSignal(signal);
						break;
					case 'exchange_status':
						const exchangeStatus = ExchangeStatusSchema.parse(message.data);
						exchangesStore.updateExchange(exchangeStatus);
						break;
					case 'system_status':
						const systemStatus = SystemStatusSchema.parse(message.data);
						systemStore.updateStatus(systemStatus);
						break;
				}
			} catch (error) {
				console.error('Failed to process WebSocket message:', error);
			}
		});

		return () => {
			unsubscribe();
			wsClient.disconnect();
		};
	});

	$effect(() => {
		document.documentElement.classList.toggle('dark', darkMode);
		localStorage.setItem('theme', darkMode ? 'dark' : 'light');
	});

	function toggleTheme(): void {
		darkMode = !darkMode;
	}
</script>

<div class="app-layout" class:sidebar-collapsed={!sidebarExpanded}>
	<Sidebar />
	
	<main class="main-content">
		<!-- Top Header -->
		<header class="top-header glass">
			<div class="header-content">
				<div class="header-left">
					<h1 class="page-title">Arbitrage Dashboard</h1>
				</div>
				
				<div class="header-actions">
					<!-- Theme Toggle -->
					<button 
						class="icon-btn"
						onclick={toggleTheme}
						aria-label={darkMode ? 'Switch to light mode' : 'Switch to dark mode'}
					>
						{#if darkMode}
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<circle cx="12" cy="12" r="5" stroke-linecap="round" stroke-linejoin="round"/>
								<path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						{:else}
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						{/if}
					</button>
					
					<!-- Notifications -->
					<button class="icon-btn" aria-label="Notifications">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 01-3.46 0" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						<span class="notification-badge">3</span>
					</button>
					
					<!-- User Avatar -->
					<div class="user-avatar">
						<span>JD</span>
					</div>
				</div>
			</div>
		</header>
		
		<!-- Page Content -->
		<div class="page-content">
			{#if isLoading}
				<div class="loading-state">
					<div class="loading-spinner"></div>
					<span class="loading-text">Loading dashboard...</span>
				</div>
			{:else}
				<slot />
			{/if}
		</div>
	</main>
</div>

<style>
	.app-layout {
		display: flex;
		min-height: 100vh;
		background-color: var(--md-sys-color-surface);
	}
	
	.main-content {
		flex: 1;
		margin-left: var(--sidebar-width);
		display: flex;
		flex-direction: column;
		transition: margin-left var(--duration-normal) var(--ease-emphasized);
	}
	
	.app-layout.sidebar-collapsed .main-content {
		margin-left: var(--sidebar-collapsed-width);
	}
	
	.top-header {
		position: sticky;
		top: 0;
		z-index: 40;
		border-bottom: 1px solid var(--glass-border);
	}
	
	.header-content {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-4) var(--space-6);
	}
	
	.page-title {
		font: var(--typography-headline-small);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}
	
	.header-actions {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	
	.icon-btn {
		width: 40px;
		height: 40px;
		border-radius: var(--shape-full);
		border: none;
		background: transparent;
		color: var(--md-sys-color-on-surface-variant);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		position: relative;
		transition: all var(--duration-fast) var(--ease-standard);
	}
	
	.icon-btn:hover {
		background-color: var(--md-sys-color-surface-container-high);
		color: var(--md-sys-color-on-surface);
	}
	
	.icon-btn svg {
		width: 20px;
		height: 20px;
	}
	
	.notification-badge {
		position: absolute;
		top: 6px;
		right: 6px;
		width: 16px;
		height: 16px;
		border-radius: var(--shape-full);
		background-color: var(--md-sys-color-error);
		color: var(--md-sys-color-on-error);
		font: var(--typography-label-sm);
		font-size: 10px;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	
	.user-avatar {
		width: 36px;
		height: 36px;
		border-radius: var(--shape-full);
		background: linear-gradient(135deg, var(--md-sys-color-primary), var(--md-sys-color-tertiary));
		color: var(--md-sys-color-on-primary);
		display: flex;
		align-items: center;
		justify-content: center;
		font: var(--typography-label-large);
		font-weight: 600;
		margin-left: var(--space-2);
	}
	
	.page-content {
		flex: 1;
		padding: var(--space-6);
		overflow-y: auto;
	}
	
	.loading-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 60vh;
		gap: var(--space-4);
	}
	
	.loading-spinner {
		width: 48px;
		height: 48px;
		border: 3px solid var(--md-sys-color-surface-container-high);
		border-top-color: var(--md-sys-color-primary);
		border-radius: var(--shape-full);
		animation: spin 1s linear infinite;
	}
	
	.loading-text {
		font: var(--typography-body-large);
		color: var(--md-sys-color-on-surface-variant);
	}
	
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
