<script lang="ts">
	import '../app.css';
	import { WebSocketClient } from '$lib/websocket/client.svelte';
	import { apiClient } from '$lib/api/client';
	import { signalsStore } from '$lib/stores/signals.svelte';
	import { exchangesStore } from '$lib/stores/exchanges.svelte';
	import { systemStore } from '$lib/stores/system.svelte';
	import { TradeSignalSchema, ExchangeStatusSchema, SystemStatusSchema } from '$lib/schemas';
	import { onMount } from 'svelte';
	import { t } from '$lib/i18n.svelte';
	import Sidebar from '$lib/components/ui/Sidebar.svelte';
	import type { TradeSignal } from '$lib/types';

	let wsClient: WebSocketClient;
	let darkMode = $state(false);
	let isLoading = $state(true);
	let notificationsOpen = $state(false);
	
	// Notifications state
	let notifications = $state<Array<{
		id: string;
		type: 'info' | 'warning' | 'success' | 'error';
		title: string;
		message: string;
		timestamp: number;
	}>>([]);

	onMount(async () => {
		// Initialize dark mode from localStorage
		const savedTheme = localStorage.getItem('triangulum_theme');
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
						addNotification({
							type: 'info',
							title: 'New Signal',
							message: `${signal.strategy}: ${signal.symbol} (${signal.profitBps > 0 ? '+' : ''}${signal.profitBps} bps)`
						});
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
		localStorage.setItem('triangulum_theme', darkMode ? 'dark' : 'light');
	});

	function toggleNotifications(): void {
		notificationsOpen = !notificationsOpen;
	}

	function closeNotifications(): void {
		notificationsOpen = false;
	}

	function addNotification(notification: Omit<typeof notifications[0], 'id' | 'timestamp'>): void {
		const newNotification = {
			...notification,
			id: crypto.randomUUID(),
			timestamp: Date.now()
		};
		notifications = [newNotification, ...notifications.slice(0, 49)];
	}

	function formatNotificationTime(timestamp: number): string {
		const seconds = Math.floor((Date.now() - timestamp) / 1000);
		if (seconds < 60) return `${seconds}s ago`;
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes}m ago`;
		const hours = Math.floor(minutes / 60);
		return `${hours}h ago`;
	}

	function getNotificationIcon(type: string): string {
		switch (type) {
			case 'success': return 'M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z';
			case 'warning': return 'M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z';
			case 'error': return 'M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z';
			default: return 'M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z';
		}
	}
</script>

<svelte:head>
	<title>Triangulum</title>
</svelte:head>

<div class="app-layout">
	<Sidebar />
	
	<main class="main-content">
		<!-- Top Header - Simplified -->
		<header class="top-header glass">
			<div class="header-content">
				<!-- System Status Indicator -->
				<div class="system-status">
					<div class="status-indicator" class:online={exchangesStore.onlineExchanges.length > 0}></div>
					<span class="status-text">{exchangesStore.onlineExchanges.length} {t('dashboard.exchangesConnected')}</span>
				</div>
				
				<div class="header-actions">
					<!-- Notifications Dropdown -->
					<div class="notifications-wrapper">
						<button 
							class="header-btn glass"
							onclick={toggleNotifications}
							aria-label={t('notifications.title')}
							aria-expanded={notificationsOpen}
							title={t('notifications.title')}
						>
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
								<path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 01-3.46 0" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
							{#if notifications.length > 0}
								<span class="notification-badge">{notifications.length > 99 ? '99+' : notifications.length}</span>
							{/if}
						</button>
						
						{#if notificationsOpen}
							<div class="notifications-dropdown glass glass-elevation-3" role="dialog" aria-label="Notifications">
								<div class="notifications-header">
									<h3 class="notifications-title">{t('notifications.title')}</h3>
									{#if notifications.length > 0}
										<button class="clear-btn" onclick={() => notifications = []}>{t('notifications.clearAll')}</button>
									{/if}
								</div>
								<div class="notifications-list">
									{#if notifications.length === 0}
										<div class="notifications-empty">
											<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
												<path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 01-3.46 0" stroke-linecap="round" stroke-linejoin="round"/>
											</svg>
											<p>{t('notifications.noNotifications')}</p>
										</div>
									{:else}
										{#each notifications as notification (notification.id)}
											<div class="notification-item {notification.type}">
												<div class="notification-icon">
													<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
														<path d={getNotificationIcon(notification.type)} stroke-linecap="round" stroke-linejoin="round"/>
													</svg>
												</div>
												<div class="notification-content">
													<p class="notification-title">{notification.title}</p>
													<p class="notification-message">{notification.message}</p>
													<span class="notification-time">{formatNotificationTime(notification.timestamp)}</span>
												</div>
											</div>
										{/each}
									{/if}
								</div>
							</div>
						{/if}
					</div>
					
					<!-- Account Icon -->
					<a href="/account" class="header-btn glass account-btn" title="Account Management" aria-label="Account">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
							<path d="M12 2L2 7l10 5 10-5-10-5z" stroke-linecap="round" stroke-linejoin="round"/>
							<path d="M2 17l10 5 10-5M2 12l10 5 10-5" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</a>
				</div>
			</div>
		</header>
		
		<!-- Page Content -->
		<div class="page-content">
			{#if isLoading}
				<div class="loading-state">
					<div class="loading-spinner"></div>
					<span class="loading-text">{t('common.loading')}</span>
				</div>
			{:else}
				<slot />
			{/if}
		</div>
	</main>
</div>

<!-- Click outside to close notifications -->
{#if notificationsOpen}
	<button class="click-outside" onclick={closeNotifications} aria-label="Close notifications"></button>
{/if}

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
		transition: margin-left var(--duration-normal) var(--ease-emphasized), width var(--duration-normal) var(--ease-emphasized);
		width: calc(100% - var(--sidebar-width));
	}
	
	/* Expand content when sidebar is collapsed */
	.app-layout:has(aside[data-sidebar-expanded="false"]) .main-content {
		margin-left: var(--sidebar-collapsed-width);
		width: calc(100% - var(--sidebar-collapsed-width));
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
		padding: var(--space-3) var(--space-5);
		gap: var(--space-4);
	}
	
	.system-status {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	
	.status-indicator {
		width: 8px;
		height: 8px;
		border-radius: var(--shape-full);
		background: var(--md-sys-color-outline);
		transition: background-color var(--duration-fast) var(--ease-standard);
	}
	
	.status-indicator.online {
		background: var(--color-profit);
		animation: pulse-live 2s ease-in-out infinite;
	}
	
	.status-text {
		font: var(--typography-label-medium);
		color: var(--md-sys-color-on-surface-variant);
	}
	
	.header-actions {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		position: relative;
	}
	
	.header-btn {
		width: 40px;
		height: 40px;
		border-radius: var(--shape-full);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
		color: var(--md-sys-color-on-surface-variant);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		position: relative;
		transition: background-color var(--duration-fast) var(--ease-standard), border-color var(--duration-fast) var(--ease-standard), color var(--duration-fast) var(--ease-standard);
		text-decoration: none;
	}
	
	.header-btn:hover {
		background: var(--glass-background-subtle);
		border-color: var(--glass-border-strong);
		color: var(--md-sys-color-on-surface);
	}
	
	.header-btn svg {
		width: 20px;
		height: 20px;
	}
	
	.notification-badge {
		position: absolute;
		top: 2px;
		right: 2px;
		min-width: 16px;
		height: 16px;
		border-radius: var(--shape-full);
		background-color: var(--md-sys-color-error);
		color: var(--md-sys-color-on-error);
		font: var(--typography-label-sm);
		font-size: 9px;
		font-weight: 600;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0 3px;
	}
	
	.notifications-wrapper {
		position: relative;
	}
	
	.notifications-dropdown {
		position: absolute;
		top: calc(100% + 8px);
		right: 0;
		width: 360px;
		max-height: 480px;
		border-radius: var(--shape-large);
		overflow: hidden;
		animation: slide-in-up var(--duration-normal) var(--ease-emphasized-decelerate);
	}
	
	@media (prefers-reduced-motion: reduce) {
		.notifications-dropdown {
			animation: fade-in var(--duration-normal) var(--ease-standard);
		}
	}
	
	.notifications-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-4);
		border-bottom: 1px solid var(--glass-border);
	}
	
	.notifications-title {
		font: var(--typography-title-medium);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}
	
	.clear-btn {
		background: none;
		border: none;
		color: var(--md-sys-color-primary);
		font: var(--typography-label-medium);
		cursor: pointer;
		padding: var(--space-1) var(--space-2);
		border-radius: var(--shape-small);
		transition: background-color var(--duration-fast) var(--ease-standard);
	}
	
	.clear-btn:hover {
		background-color: var(--md-sys-color-surface-container-high);
	}
	
	.notifications-list {
		max-height: 400px;
		overflow-y: auto;
	}
	
	.notifications-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--space-10) var(--space-6);
		color: var(--md-sys-color-on-surface-variant);
		text-align: center;
	}
	
	.notifications-empty svg {
		width: 48px;
		height: 48px;
		margin-bottom: var(--space-4);
		opacity: 0.5;
	}
	
	.notifications-empty p {
		font: var(--typography-body-medium);
		margin: 0;
	}
	
	.notification-item {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		padding: var(--space-4);
		border-bottom: 1px solid var(--glass-border);
		transition: background-color var(--duration-fast) var(--ease-standard);
	}
	
	.notification-item:hover {
		background-color: var(--md-sys-color-surface-container-low);
	}
	
	.notification-item:last-child {
		border-bottom: none;
	}
	
	.notification-icon {
		width: 32px;
		height: 32px;
		border-radius: var(--shape-full);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	
	.notification-icon svg {
		width: 16px;
		height: 16px;
	}
	
	.notification-item.info .notification-icon {
		background-color: var(--md-sys-color-primary-container);
		color: var(--md-sys-color-on-primary-container);
	}
	
	.notification-item.success .notification-icon {
		background-color: var(--color-profit-container);
		color: var(--color-profit);
	}
	
	.notification-item.warning .notification-icon {
		background-color: var(--color-warning-container);
		color: var(--color-warning);
	}
	
	.notification-item.error .notification-icon {
		background-color: var(--md-sys-color-error-container);
		color: var(--md-sys-color-error);
	}
	
	.notification-content {
		flex: 1;
		min-width: 0;
	}
	
	.notification-title {
		font: var(--typography-label-medium);
		color: var(--md-sys-color-on-surface);
		font-weight: 500;
		margin: 0 0 var(--space-1) 0;
	}
	
	.notification-message {
		font: var(--typography-body-small);
		color: var(--md-sys-color-on-surface-variant);
		margin: 0 0 var(--space-1) 0;
		word-break: break-word;
	}
	
	.notification-time {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
		opacity: 0.7;
	}
	
	.click-outside {
		position: fixed;
		inset: 0;
		z-index: 30;
		background: transparent;
		border: none;
		cursor: default;
	}
	
	@keyframes slide-in-up {
		from {
			opacity: 0;
			transform: translateY(10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
	
	@keyframes fade-in {
		from { opacity: 0; }
		to { opacity: 1; }
	}
	
	@keyframes pulse-live {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.5; }
	}
</style>
