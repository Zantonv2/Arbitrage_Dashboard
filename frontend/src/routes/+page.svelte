<script lang="ts">
	import SignalCard from '$lib/components/SignalCard.svelte';
	import ExchangeCard from '$lib/components/ExchangeCard.svelte';
	import Card from '$lib/components/ui/Card.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import { signalsStore } from '$lib/stores/signals.svelte';
	import { exchangesStore } from '$lib/stores/exchanges.svelte';
	import { t } from '$lib/i18n.svelte';
	import type { TradeSignal } from '$lib/types';

	// Generate example signals for demonstration
	const exampleSignals: TradeSignal[] = [
		{
			id: 'sig_001',
			symbol: 'BTC/USDT',
			buyExchange: 'okx',
			sellExchange: 'bybit',
			buyPrice: 43250.00,
			sellPrice: 43320.00,
			profitBps: 16.2,
			confidence: 0.87,
			timestamp: Date.now() - 30000,
			strategy: 'cex-arbitrage',
			volume: 2.5,
			fees: 0.1
		},
		{
			id: 'sig_002',
			symbol: 'ETH/USDT',
			buyExchange: 'gateio',
			sellExchange: 'mexc',
			buyPrice: 2280.50,
			sellPrice: 2285.75,
			profitBps: 23.0,
			confidence: 0.92,
			timestamp: Date.now() - 120000,
			strategy: 'funding-arbitrage',
			volume: 15.0,
			fees: 0.15
		},
		{
			id: 'sig_003',
			symbol: 'SOL/USDT',
			buyExchange: 'bybit',
			sellExchange: 'okx',
			buyPrice: 98.45,
			sellPrice: 98.90,
			profitBps: 45.7,
			confidence: 0.78,
			timestamp: Date.now() - 240000,
			strategy: 'spot-perp-arbitrage',
			volume: 150.0,
			fees: 0.2
		},
		{
			id: 'sig_004',
			symbol: 'USDC/USDT',
			buyExchange: 'kraken',
			sellExchange: 'bitstamp',
			buyPrice: 0.9998,
			sellPrice: 1.0001,
			profitBps: 3.0,
			confidence: 0.95,
			timestamp: Date.now() - 450000,
			strategy: 'stablecoin-arbitrage',
			volume: 50000.0,
			fees: 0.05
		},
		{
			id: 'sig_005',
			symbol: 'XRP/USDT',
			buyExchange: 'mexc',
			sellExchange: 'gateio',
			buyPrice: 0.5125,
			sellPrice: 0.5140,
			profitBps: 29.3,
			confidence: 0.71,
			timestamp: Date.now() - 600000,
			strategy: 'latency-arbitrage',
			volume: 5000.0,
			fees: 0.12
		},
		{
			id: 'sig_006',
			symbol: 'ADA/USDT',
			buyExchange: 'okx',
			sellExchange: 'bybit',
			buyPrice: 0.4580,
			sellPrice: 0.4625,
			profitBps: 98.2,
			confidence: 0.65,
			timestamp: Date.now() - 800000,
			strategy: 'cross-exchange-arbitrage',
			volume: 8000.0,
			fees: 0.18
		}
	];

	// Use example signals if no real signals exist
	const displaySignals = $derived(
		signalsStore.sortedSignals.length > 0 ? signalsStore.sortedSignals.slice(0, 6) : exampleSignals
	);

	function handleSignalSelect(signal: TradeSignal): void {
		console.log('Selected signal:', signal);
	}

	function handleSignalExecute(signal: TradeSignal): void {
		console.log('Execute signal:', signal);
	}

	const stats = $derived({
		totalSignals: signalsStore.sortedSignals.length > 0 ? signalsStore.sortedSignals.length : exampleSignals.length,
		profitableSignals: displaySignals.filter(s => s.profitBps > 0).length,
		onlineExchanges: exchangesStore.onlineExchanges.length,
		totalExchanges: exchangesStore.allExchanges.length,
		avgProfit: displaySignals.reduce((acc, s) => acc + s.profitBps, 0) / displaySignals.length
	});
</script>

<svelte:head>
	<title>Dashboard - Triangulum</title>
</svelte:head>

<div class="dashboard">
	<!-- KPI Cards Row - Metrics Display (Pure Data, Not Interactive) -->
	<section class="kpi-section">
		<div class="kpi-grid">
			<!-- KPI Card 1: Total Signals -->
			<div class="kpi-card">
				<div class="kpi-display glass">
					<div class="kpi-header">
						<div class="kpi-icon-wrapper bg-primary-soft">
							<svg class="kpi-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
								<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</div>
					<div class="kpi-content">
						<div class="kpi-value">{stats.totalSignals}</div>
						<div class="kpi-label">{t('dashboard.kpiTotalSignals')}</div>
					</div>
					<div class="kpi-trend positive">
						<svg class="trend-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
							<path d="M7 17l5-5 5 5M12 12V3" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						<span>+12%</span>
					</div>
				</div>
			</div>
			
			<!-- KPI Card 2: Profitable Opportunities -->
			<div class="kpi-card">
				<div class="kpi-display glass">
					<div class="kpi-header">
						<div class="kpi-icon-wrapper bg-profit-soft">
							<svg class="kpi-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
								<path d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</div>
					<div class="kpi-content">
						<div class="kpi-value text-profit">{stats.profitableSignals}</div>
						<div class="kpi-label">{t('dashboard.kpiProfitable')}</div>
					</div>
					<div class="kpi-badges">
						<Badge variant="tonal" color="profit" size="small">
							{stats.totalSignals > 0 ? Math.round((stats.profitableSignals / stats.totalSignals) * 100) : 0}% rate
						</Badge>
					</div>
				</div>
			</div>
			
			<!-- KPI Card 3: Online Exchanges -->
			<div class="kpi-card">
				<div class="kpi-display glass">
					<div class="kpi-header">
						<div class="kpi-icon-wrapper bg-secondary-soft">
							<svg class="kpi-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
								<path d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</div>
					<div class="kpi-content">
						<div class="kpi-value">{stats.onlineExchanges > 0 ? stats.onlineExchanges : 6}<span class="kpi-divider">/</span>{stats.totalExchanges > 0 ? stats.totalExchanges : 6}</div>
						<div class="kpi-label">{t('dashboard.kpiOnlineExchanges')}</div>
					</div>
					<div class="kpi-badges">
						<Badge variant="tonal" color="profit" size="small">All Operational</Badge>
					</div>
				</div>
			</div>
			
			<!-- KPI Card 4: Avg Profit -->
			<div class="kpi-card">
				<div class="kpi-display glass">
					<div class="kpi-header">
						<div class="kpi-icon-wrapper bg-tertiary-soft">
							<svg class="kpi-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
								<path d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</div>
					<div class="kpi-content">
						<div class="kpi-value text-profit">
							+{stats.avgProfit.toFixed(2)}
						</div>
						<div class="kpi-label">{t('dashboard.kpiAvgProfit')}</div>
					</div>
					<div class="kpi-subtitle">Per signal average</div>
				</div>
			</div>
		</div>
	</section>

	<!-- Exchange Status -->
	<section class="section">
		<div class="section-header">
			<h2 class="section-title">{t('dashboard.exchangeStatus')}</h2>
			<a href="/exchanges" class="view-all-link glass" aria-label="View all exchanges">
				{t('dashboard.viewAll')}
				<svg class="link-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
					<path d="M9 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</a>
		</div>
		<div class="exchange-grid">
			<ExchangeCard status={{ exchange: 'okx', status: 'online', lastUpdate: Date.now(), latency: 45, rateLimitUsage: 0.32 }} />
			<ExchangeCard status={{ exchange: 'bybit', status: 'online', lastUpdate: Date.now() - 5000, latency: 62, rateLimitUsage: 0.28 }} />
			<ExchangeCard status={{ exchange: 'gateio', status: 'online', lastUpdate: Date.now() - 10000, latency: 89, rateLimitUsage: 0.45 }} />
		</div>
	</section>

	<!-- Top Signals -->
	<section class="section">
		<div class="section-header">
			<h2 class="section-title">{t('dashboard.topOpportunities')}</h2>
			<a href="/signals" class="view-all-link glass" aria-label="View all signals">
				{t('dashboard.viewAll')}
				<svg class="link-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
					<path d="M9 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</a>
		</div>
		<div class="signals-grid">
			{#each displaySignals as signal (signal.id)}
				<SignalCard 
					signal={signal} 
					variant="detailed"
					onSelect={handleSignalSelect}
					onExecute={handleSignalExecute}
				/>
			{/each}
		</div>
	</section>
</div>

<style>
	.dashboard {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}

	/* KPI Section - Metrics Display (Not Interactive) */
	.kpi-section {
		width: 100%;
	}

	.kpi-grid {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: var(--space-4);
	}

	.kpi-card {
		min-height: 140px;
	}

	.kpi-display {
		height: 100%;
		display: flex;
		flex-direction: column;
		padding: var(--space-5);
		border-radius: var(--shape-extra-large);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
		transition: border-color var(--duration-normal) var(--ease-standard), box-shadow var(--duration-normal) var(--ease-standard);
	}

	.kpi-display:hover {
		border-color: var(--glass-border-strong);
		box-shadow: var(--glass-elevation-2);
	}

	.kpi-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		margin-bottom: var(--space-3);
	}

	.kpi-icon-wrapper {
		width: 44px;
		height: 44px;
		border-radius: var(--shape-medium);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.kpi-icon {
		width: 22px;
		height: 22px;
	}

	.bg-primary-soft {
		background-color: var(--md-sys-color-primary-container);
		color: var(--md-sys-color-on-primary-container);
	}

	.bg-profit-soft {
		background-color: var(--color-profit-container);
		color: var(--color-profit);
	}

	.bg-secondary-soft {
		background-color: var(--md-sys-color-secondary-container);
		color: var(--md-sys-color-on-secondary-container);
	}

	.bg-tertiary-soft {
		background-color: var(--md-sys-color-tertiary-container);
		color: var(--md-sys-color-on-tertiary-container);
	}

	.kpi-content {
		flex: 1;
	}

	.kpi-value {
		font: var(--typography-headline-medium);
		font-weight: 500;
		color: var(--md-sys-color-on-surface);
		line-height: 1.2;
	}

	.kpi-divider {
		color: var(--md-sys-color-on-surface-variant);
		margin: 0 var(--space-1);
	}

	.kpi-label {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
		margin-top: var(--space-1);
	}

	.kpi-trend {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		font: var(--typography-label-medium);
		margin-top: var(--space-3);
	}

	.kpi-trend.positive {
		color: var(--color-profit);
	}

	.trend-icon {
		width: 16px;
		height: 16px;
	}

	.kpi-badges {
		margin-top: var(--space-3);
	}

	.kpi-subtitle {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
		margin-top: var(--space-2);
	}

	.text-profit {
		color: var(--color-profit);
	}

	/* Section Styles */
	.section {
		width: 100%;
	}

	.section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--space-4);
	}

	.section-title {
		font: var(--typography-title-large);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.view-all-link {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		color: var(--md-sys-color-primary);
		text-decoration: none;
		font: var(--typography-label-large);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--shape-full);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
		transition: background-color var(--duration-fast) var(--ease-standard), border-color var(--duration-fast) var(--ease-standard);
	}

	.view-all-link:hover {
		background: var(--glass-background-subtle);
		border-color: var(--glass-border-strong);
	}

	.link-icon {
		width: 16px;
		height: 16px;
	}

	/* Exchange Grid */
	.exchange-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--space-4);
	}

	/* Signals Grid */
	.signals-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--space-4);
	}

	/* Responsive */
	@media (max-width: 1200px) {
		.kpi-grid {
			grid-template-columns: repeat(2, 1fr);
		}

		.exchange-grid,
		.signals-grid {
			grid-template-columns: repeat(2, 1fr);
		}
	}

	@media (max-width: 768px) {
		.kpi-grid {
			grid-template-columns: 1fr;
		}

		.exchange-grid,
		.signals-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
