<script lang="ts">
	import ExchangeCard from '$lib/components/ExchangeCard.svelte';
	import Card from '$lib/components/ui/Card.svelte';
	import { exchangesStore } from '$lib/stores/exchanges.svelte';

	// KPI metrics
	const stats = $derived({
		online: exchangesStore.onlineExchanges.length,
		offline: exchangesStore.offlineExchanges.length,
		total: exchangesStore.allExchanges.length
	});
</script>

<svelte:head>
	<title>Exchanges - Arbitrage</title>
</svelte:head>

<div class="exchanges-page">
	<h1 class="page-title">Exchange Status</h1>

	<!-- Stats Row -->
	<div class="stats-grid">
		<div class="stat-card glass">
			<div class="stat-icon online">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</div>
			<div class="stat-content">
				<div class="stat-value text-profit">{stats.online}</div>
				<div class="stat-label">Online</div>
			</div>
		</div>

		<div class="stat-card glass">
			<div class="stat-icon offline">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M18.364 5.636a9 9 0 11-12.728 0M12 9v4" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</div>
			<div class="stat-content">
				<div class="stat-value text-loss">{stats.offline}</div>
				<div class="stat-label">Offline</div>
			</div>
		</div>

		<div class="stat-card glass">
			<div class="stat-icon total">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</div>
			<div class="stat-content">
				<div class="stat-value">{stats.total}</div>
				<div class="stat-label">Total</div>
			</div>
		</div>
	</div>

	<!-- Exchange Grid -->
	<div class="exchange-grid">
		{#each exchangesStore.allExchanges as exchange (exchange.exchange)}
			<ExchangeCard status={exchange} />
		{:else}
			<div class="empty-state-wrapper">
				<div class="empty-state-card">
					<div class="empty-icon-wrapper">
						<svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
							<path d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</div>
					<p class="empty-title">No exchange data available</p>
					<p class="empty-subtitle">Connect exchanges in settings to view status</p>
				</div>
			</div>
		{/each}
	</div>
</div>

<style>
	.exchanges-page {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}

	.page-title {
		font: var(--typography-headline-medium);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.stats-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--space-4);
	}

	.stat-card {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		padding: var(--space-5);
		border-radius: var(--shape-large);
	}

	.stat-icon {
		width: 48px;
		height: 48px;
		border-radius: var(--shape-medium);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.stat-icon svg {
		width: 24px;
		height: 24px;
	}

	.stat-icon.online {
		background: var(--color-profit-container);
		color: var(--color-profit);
	}

	.stat-icon.offline {
		background: var(--md-sys-color-error-container);
		color: var(--md-sys-color-error);
	}

	.stat-icon.total {
		background: var(--md-sys-color-primary-container);
		color: var(--md-sys-color-on-primary-container);
	}

	.stat-content {
		flex: 1;
	}

	.stat-value {
		font: var(--typography-display-small);
		font-weight: 500;
		color: var(--md-sys-color-on-surface);
		line-height: 1;
	}

	.stat-label {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
		margin-top: var(--space-1);
	}

	.text-profit {
		color: var(--color-profit);
	}

	.text-loss {
		color: var(--md-sys-color-error);
	}

	.exchange-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--space-4);
	}

	.empty-state-wrapper {
		grid-column: 1 / -1;
		display: flex;
		justify-content: center;
		align-items: center;
		min-height: 300px;
	}

	.empty-state-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		padding: var(--space-10);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
		border: 1px solid var(--glass-border);
		border-radius: var(--shape-extra-large);
	}

	.empty-icon-wrapper {
		width: 80px;
		height: 80px;
		border-radius: var(--shape-full);
		background: var(--md-sys-color-surface-container-high);
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: var(--space-5);
	}

	.empty-icon {
		width: 40px;
		height: 40px;
		color: var(--md-sys-color-on-surface-variant);
	}

	.empty-title {
		font: var(--typography-title-large);
		color: var(--md-sys-color-on-surface);
		margin: 0 0 var(--space-2) 0;
	}

	.empty-subtitle {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
		margin: 0;
	}

	@media (max-width: 1024px) {
		.exchange-grid {
			grid-template-columns: repeat(2, 1fr);
		}
	}

	@media (max-width: 768px) {
		.stats-grid {
			grid-template-columns: 1fr;
		}

		.exchange-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
