<script lang="ts">
	import SignalCard from '$lib/components/SignalCard.svelte';
	import { signalsStore } from '$lib/stores/signals.svelte';
	import type { TradeSignal, StrategyType } from '$lib/types';
	import Badge from '$lib/components/ui/Badge.svelte';

	// Default position size for profit calculation
	const DEFAULT_POSITION_SIZE = 10000;

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
		},
		{
			id: 'sig_007',
			symbol: 'DOGE/USDT',
			buyExchange: 'gateio',
			sellExchange: 'mexc',
			buyPrice: 0.0825,
			sellPrice: 0.0831,
			profitBps: 72.7,
			confidence: 0.68,
			timestamp: Date.now() - 900000,
			strategy: 'spread-capture',
			volume: 100000.0,
			fees: 0.25
		},
		{
			id: 'sig_008',
			symbol: 'LINK/USDT',
			buyExchange: 'bybit',
			sellExchange: 'okx',
			buyPrice: 14.85,
			sellPrice: 14.92,
			profitBps: 47.1,
			confidence: 0.82,
			timestamp: Date.now() - 1000000,
			strategy: 'convergence-arbitrage',
			volume: 2000.0,
			fees: 0.1
		}
	];

	let sortBy = $state<'profit' | 'confidence' | 'time'>('profit');
	let sortOrder = $state<'asc' | 'desc'>('desc');

	// Use example signals if no real signals exist
	const allSignals = $derived(
		signalsStore.sortedSignals.length > 0 ? signalsStore.sortedSignals : exampleSignals
	);

	const sortedSignals = $derived(() => {
		const signals = [...allSignals];
		
		signals.sort((a, b) => {
			let comparison = 0;
			switch (sortBy) {
				case 'profit':
					// Sort by actual profit (profitBps * position_size / 10000)
					const profitA = (a.profitBps * DEFAULT_POSITION_SIZE) / 10000;
					const profitB = (b.profitBps * DEFAULT_POSITION_SIZE) / 10000;
					comparison = profitA - profitB;
					break;
				case 'confidence':
					comparison = a.confidence - b.confidence;
					break;
				case 'time':
					comparison = a.timestamp - b.timestamp;
					break;
			}
			return sortOrder === 'asc' ? comparison : -comparison;
		});

		return signals;
	});

	function handleSignalSelect(signal: TradeSignal): void {
		console.log('Selected signal:', signal);
	}

	function handleSort(column: 'profit' | 'confidence' | 'time'): void {
		if (sortBy === column) {
			sortOrder = sortOrder === 'asc' ? 'desc' : 'asc';
		} else {
			sortBy = column;
			sortOrder = 'desc';
		}
	}

	const strategyOptions: { value: StrategyType | null; label: string }[] = [
		{ value: null, label: 'All Strategies' },
		{ value: 'cex-arbitrage', label: 'CEX Arbitrage' },
		{ value: 'funding-arbitrage', label: 'Funding Rate Arbitrage' },
		{ value: 'spot-perp-arbitrage', label: 'Spot-Perp Arbitrage' },
		{ value: 'stablecoin-arbitrage', label: 'Stablecoin Arbitrage' },
		{ value: 'cross-exchange-arbitrage', label: 'Cross-Exchange Arbitrage' },
		{ value: 'spread-capture', label: 'Spread Capture' },
		{ value: 'latency-arbitrage', label: 'Latency Arbitrage' },
		{ value: 'convergence-arbitrage', label: 'Convergence Arbitrage' }
	];

	let selectedStrategy = $state<StrategyType | null>(null);

	const filteredSignals = $derived(() => {
		const signals = sortedSignals();
		if (selectedStrategy) {
			return signals.filter(s => s.strategy === selectedStrategy);
		}
		return signals;
	});
</script>

<svelte:head>
	<title>Signals - Triangulum</title>
</svelte:head>

<div class="signals-page">
	<div class="page-header">
		<h1 class="page-title">Trade Signals</h1>
		<Badge variant="tonal" color="default" size="medium">
			{filteredSignals().length} signals
		</Badge>
	</div>

	<!-- Filter and Sort Bar -->
	<div class="filter-bar glass">
		<div class="filter-row">
			<div class="filter-group">
				<label for="strategy-filter" class="filter-label">Filter by Strategy</label>
				<select
					id="strategy-filter"
					bind:value={selectedStrategy}
					class="filter-select glass-input"
				>
					{#each strategyOptions as option}
						<option value={option.value}>{option.label}</option>
					{/each}
				</select>
			</div>

			<div class="sort-group">
				<span class="sort-label">Sort by</span>
				<div class="sort-buttons">
					<button
						class="sort-btn glass-interactive"
						class:active={sortBy === 'profit'}
						onclick={() => handleSort('profit')}
						aria-sort={sortBy === 'profit' ? sortOrder === 'asc' ? 'ascending' : 'descending' : 'none'}
					>
						Profit
						{#if sortBy === 'profit'}
							<span class="sort-indicator">{sortOrder === 'asc' ? '↑' : '↓'}</span>
						{/if}
					</button>
					<button
						class="sort-btn glass-interactive"
						class:active={sortBy === 'confidence'}
						onclick={() => handleSort('confidence')}
						aria-sort={sortBy === 'confidence' ? sortOrder === 'asc' ? 'ascending' : 'descending' : 'none'}
					>
						Confidence
						{#if sortBy === 'confidence'}
							<span class="sort-indicator">{sortOrder === 'asc' ? '↑' : '↓'}</span>
						{/if}
					</button>
					<button
						class="sort-btn glass-interactive"
						class:active={sortBy === 'time'}
						onclick={() => handleSort('time')}
						aria-sort={sortBy === 'time' ? sortOrder === 'asc' ? 'ascending' : 'descending' : 'none'}
					>
						Time
						{#if sortBy === 'time'}
							<span class="sort-indicator">{sortOrder === 'asc' ? '↑' : '↓'}</span>
						{/if}
					</button>
				</div>
			</div>
		</div>
	</div>

	<!-- Signals Grid -->
	<div class="signals-grid" role="list">
		{#each filteredSignals() as signal (signal.id)}
			<div role="listitem">
				<SignalCard {signal} variant="detailed" onSelect={handleSignalSelect} />
			</div>
		{:else}
			<div class="empty-state glass">
				<div class="empty-state-content">
					<svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
						<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
					<p class="empty-title">No signals match your filters</p>
					<p class="empty-subtitle">Try adjusting your filter criteria</p>
				</div>
			</div>
		{/each}
	</div>
</div>

<style>
	.signals-page {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}

	.page-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.page-title {
		font: var(--typography-headline-medium);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.filter-bar {
		padding: var(--space-4);
		border-radius: var(--shape-extra-large);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
	}

	.filter-row {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-6);
		flex-wrap: wrap;
	}

	.filter-group {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.filter-label {
		font: var(--typography-label-medium);
		color: var(--md-sys-color-on-surface-variant);
	}

	.filter-select {
		padding: var(--space-2) var(--space-4);
		border: 1px solid var(--glass-border);
		border-radius: var(--shape-large);
		background: var(--glass-background);
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
		min-width: 200px;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.filter-select:focus {
		outline: none;
		border-color: var(--md-sys-color-primary);
	}

	.sort-group {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.sort-label {
		font: var(--typography-label-medium);
		color: var(--md-sys-color-on-surface-variant);
	}

	.sort-buttons {
		display: flex;
		gap: var(--space-2);
	}

	.sort-btn {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--glass-border);
		border-radius: var(--shape-full);
		background: transparent;
		font: var(--typography-label-medium);
		color: var(--md-sys-color-on-surface-variant);
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.sort-btn:hover {
		background: var(--glass-background-subtle);
		border-color: var(--glass-border-strong);
	}

	.sort-btn.active {
		background: var(--md-sys-color-primary-container);
		border-color: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary-container);
	}

	.sort-indicator {
		font-size: 12px;
	}

	.signals-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
		gap: var(--space-4);
	}

	.empty-state {
		grid-column: 1 / -1;
		display: flex;
		justify-content: center;
		align-items: center;
		min-height: 300px;
		border-radius: var(--shape-extra-large);
		border: 1px solid var(--glass-border);
	}

	.empty-state-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		padding: var(--space-8);
	}

	.empty-icon {
		width: 64px;
		height: 64px;
		color: var(--md-sys-color-on-surface-variant);
		opacity: 0.5;
		margin-bottom: var(--space-4);
	}

	.empty-title {
		font: var(--typography-title-medium);
		color: var(--md-sys-color-on-surface);
		margin: 0 0 var(--space-2) 0;
	}

	.empty-subtitle {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
		margin: 0;
	}

	@media (max-width: 768px) {
		.filter-row {
			flex-direction: column;
			align-items: stretch;
		}

		.sort-buttons {
			flex-wrap: wrap;
		}
	}
</style>
