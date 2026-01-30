<script lang="ts">
	import SignalCard from '$lib/components/SignalCard.svelte';
	import ExchangeCard from '$lib/components/ExchangeCard.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Card from '$lib/components/ui/Card.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import { signalsStore } from '$lib/stores/signals.svelte';
	import { exchangesStore } from '$lib/stores/exchanges.svelte';
	import { systemStore } from '$lib/stores/system.svelte';
	import type { TradeSignal } from '$lib/types';

	function handleSignalSelect(signal: TradeSignal): void {
		console.log('Selected signal:', signal);
	}

	function handleSignalExecute(signal: TradeSignal): void {
		console.log('Execute signal:', signal);
	}

	function toggleExecutionMode() {
		systemStore.setMode(systemStore.mode === 'auto' ? 'manual' : 'auto');
	}

	const topSignals = $derived(signalsStore.sortedSignals.slice(0, 6));
	
	const stats = $derived({
		totalSignals: signalsStore.sortedSignals.length,
		profitableSignals: signalsStore.sortedSignals.filter(s => s.profitBps > 0).length,
		onlineExchanges: exchangesStore.onlineExchanges.length,
		totalExchanges: exchangesStore.allExchanges.length,
		avgProfit: signalsStore.sortedSignals.length > 0 
			? signalsStore.sortedSignals.reduce((acc, s) => acc + s.profitBps, 0) / signalsStore.sortedSignals.length 
			: 0
	});
</script>

<svelte:head>
	<title>Dashboard - Arbitrage</title>
</svelte:head>

<div class="dashboard">
	<!-- KPI Cards Row - Display Only, Not Buttons -->
	<section class="kpi-section">
		<div class="kpi-grid">
			<!-- KPI Card 1: Total Signals -->
			<div class="kpi-card">
				<div class="kpi-glass">
					<div class="kpi-header">
						<div class="kpi-icon-wrapper bg-primary-soft">
							<svg class="kpi-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</div>
					<div class="kpi-value">{stats.totalSignals}</div>
					<div class="kpi-label">Total Signals</div>
					<div class="kpi-trend positive">
						<svg class="trend-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M7 17l5-5 5 5M12 12V3" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						<span>+12%</span>
					</div>
				</div>
			</div>
			
			<!-- KPI Card 2: Profitable Opportunities -->
			<div class="kpi-card">
				<div class="kpi-glass profit-glow">
					<div class="kpi-header">
						<div class="kpi-icon-wrapper bg-profit-soft">
							<svg class="kpi-icon text-profit" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</div>
					<div class="kpi-value text-profit">{stats.profitableSignals}</div>
					<div class="kpi-label">Profitable</div>
					<div class="kpi-badge-row">
						<Badge variant="tonal" color="profit" size="small">
							{stats.totalSignals > 0 ? Math.round((stats.profitableSignals / stats.totalSignals) * 100) : 0}% rate
						</Badge>
					</div>
				</div>
			</div>
			
			<!-- KPI Card 3: Online Exchanges -->
			<div class="kpi-card">
				<div class="kpi-glass">
					<div class="kpi-header">
						<div class="kpi-icon-wrapper bg-secondary-soft">
							<svg class="kpi-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</div>
					<div class="kpi-value">{stats.onlineExchanges}/{stats.totalExchanges}</div>
					<div class="kpi-label">Exchanges Online</div>
					<div class="kpi-badge-row">
						{#if stats.onlineExchanges === stats.totalExchanges}
							<Badge variant="tonal" color="profit" size="small">All Operational</Badge>
						{:else}
							<Badge variant="tonal" color="warning" size="small">{stats.totalExchanges - stats.onlineExchanges} Down</Badge>
						{/if}
					</div>
				</div>
			</div>
			
			<!-- KPI Card 4: Avg Profit -->
			<div class="kpi-card">
				<div class="kpi-glass">
					<div class="kpi-header">
						<div class="kpi-icon-wrapper bg-tertiary-soft">
							<svg class="kpi-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</div>
					<div class="kpi-value {stats.avgProfit > 0 ? 'text-profit' : 'text-loss'}">
						{stats.avgProfit > 0 ? '+' : ''}{stats.avgProfit.toFixed(2)}
					</div>
					<div class="kpi-label">Avg Profit (bps)</div>
					<div class="kpi-subtitle">Per signal average</div>
				</div>
			</div>
		</div>
	</section>

	<!-- Control Panel with Execution Mode Toggle -->
	<section class="control-section">
		<div class="control-glass">
			<div class="control-content">
				<!-- Left: Execution Mode Selector -->
				<div class="mode-selector">
					<span class="mode-label">Execution Mode</span>
					<div class="mode-toggle" role="radiogroup" aria-label="Execution mode">
						<button 
							class="mode-option"
							class:active={systemStore.mode === 'manual'}
							onclick={() => systemStore.setMode('manual')}
							role="radio"
							aria-checked={systemStore.mode === 'manual'}
						>
							<div class="mode-icon">
								<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M12 11c0 3.517-1.009 6.799-2.753 9.571m-3.44-2.04l.054-.09A13.916 13.916 0 008 11a4 4 0 118 0c0 1.017-.07 2.019-.203 3m-2.118 6.844A21.88 21.88 0 0015.171 17m3.839 1.132c.645-2.266.99-4.659.99-7.132A8 8 0 008 4.07M3 15.364c.64-1.319 1-2.8 1-4.364 0-1.457.39-2.823 1.07-4" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
							</div>
							<span class="mode-text">Manual</span>
							{#if systemStore.mode === 'manual'}
								<div class="mode-indicator"></div>
							{/if}
						</button>
						<button 
							class="mode-option"
							class:active={systemStore.mode === 'auto'}
							onclick={() => systemStore.setMode('auto')}
							role="radio"
							aria-checked={systemStore.mode === 'auto'}
						>
							<div class="mode-icon">
								<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
							</div>
							<span class="mode-text">Automatic</span>
							{#if systemStore.mode === 'auto'}
								<div class="mode-indicator"></div>
							{/if}
						</button>
					</div>
				</div>
				
				<!-- Right: Action Buttons -->
				<div class="action-buttons">
					<Button variant="elevated" size="medium">
						<svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						Configure
					</Button>
					<Button variant="filled" size="medium" class="emergency-btn">
						<svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						Emergency Stop
					</Button>
				</div>
			</div>
		</div>
	</section>

	<!-- Exchange Status -->
	<section class="section">
		<div class="section-header">
			<h2 class="section-title">Exchange Status</h2>
			<a href="/exchanges" class="view-all-link">
				View All
				<svg class="link-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M9 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</a>
		</div>
		<div class="exchange-grid">
			{#each exchangesStore.allExchanges.slice(0, 3) as exchange (exchange.exchange)}
				<ExchangeCard status={exchange} />
			{:else}
				<div class="empty-state-card">
					<div class="empty-state-content">
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
	</section>

	<!-- Top Signals -->
	<section class="section">
		<div class="section-header">
			<h2 class="section-title">Top Opportunities</h2>
			<a href="/signals" class="view-all-link">
				View All
				<svg class="link-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M9 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
			</a>
		</div>
		<div class="signals-grid">
			{#each topSignals as signal (signal.id)}
				<SignalCard 
					signal={signal} 