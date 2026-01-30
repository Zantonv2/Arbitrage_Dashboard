<script lang="ts">
	import SignalCard from '$lib/components/SignalCard.svelte';
	import { signalsStore } from '$lib/stores/signals.svelte';
	import type { TradeSignal, StrategyType } from '$lib/types';

	let sortBy = $state<'profit' | 'confidence' | 'time'>('profit');
	let sortOrder = $state<'asc' | 'desc'>('desc');

	const sortedSignals = $derived(() => {
		const signals = [...signalsStore.filteredSignals];
		
		signals.sort((a, b) => {
			let comparison = 0;
			switch (sortBy) {
				case 'profit':
					comparison = a.profitBps - b.profitBps;
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
</script>

<svelte:head>
	<title>Signals - Arbitrage Dashboard</title>
</svelte:head>

<div class="signals-page">
	<div class="flex items-center justify-between mb-6">
		<h1 class="text-3xl font-bold">Trade Signals</h1>
		<div class="text-sm text-gray-600 dark:text-gray-400">
			{sortedSignals().length} signals
		</div>
	</div>

	<div class="bg-white dark:bg-gray-800 rounded-lg p-4 mb-6 border border-gray-200 dark:border-gray-700">
		<div class="flex items-center gap-4">
			<label for="strategy-filter" class="text-sm font-medium">Filter by Strategy:</label>
			<select
				id="strategy-filter"
				bind:value={signalsStore.selectedStrategy}
				class="px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-sm"
			>
				<option value={null}>All Strategies</option>
				<option value="cex-arbitrage">CEX Arbitrage</option>
				<option value="funding-arbitrage">Funding Rate Arbitrage</option>
				<option value="spot-perp-arbitrage">Spot-Perp Arbitrage</option>
				<option value="stablecoin-arbitrage">Stablecoin Arbitrage</option>
				<option value="cross-exchange-arbitrage">Cross-Exchange Arbitrage</option>
				<option value="spread-capture">Spread Capture</option>
				<option value="latency-arbitrage">Latency Arbitrage</option>
				<option value="convergence-arbitrage">Convergence Arbitrage</option>
				<option value="new-listing-arbitrage">New Listing Arbitrage</option>
				<option value="hedged-funding">Hedged Funding</option>
			</select>

			<div class="flex gap-2 ml-auto">
				<button
					onclick={() => handleSort('profit')}
					class="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 {sortBy === 'profit' ? 'bg-blue-50 dark:bg-blue-900/20 border-blue-500' : ''}"
					aria-sort={sortBy === 'profit' ? sortOrder === 'asc' ? 'ascending' : 'descending' : 'none'}
				>
					Profit {sortBy === 'profit' ? (sortOrder === 'asc' ? '↑' : '↓') : ''}
				</button>
				<button
					onclick={() => handleSort('confidence')}
					class="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 {sortBy === 'confidence' ? 'bg-blue-50 dark:bg-blue-900/20 border-blue-500' : ''}"
					aria-sort={sortBy === 'confidence' ? sortOrder === 'asc' ? 'ascending' : 'descending' : 'none'}
				>
					Confidence {sortBy === 'confidence' ? (sortOrder === 'asc' ? '↑' : '↓') : ''}
				</button>
				<button
					onclick={() => handleSort('time')}
					class="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 {sortBy === 'time' ? 'bg-blue-50 dark:bg-blue-900/20 border-blue-500' : ''}"
					aria-sort={sortBy === 'time' ? sortOrder === 'asc' ? 'ascending' : 'descending' : 'none'}
				>
					Time {sortBy === 'time' ? (sortOrder === 'asc' ? '↑' : '↓') : ''}
				</button>
			</div>
		</div>
	</div>

	<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4" role="list">
		{#each sortedSignals() as signal (signal.id)}
			<div role="listitem">
				<SignalCard {signal} variant="detailed" onSelect={handleSignalSelect} />
			</div>
		{:else}
			<div class="col-span-full text-center text-gray-500 dark:text-gray-400 py-12">
				No signals match your filters
			</div>
		{/each}
	</div>
</div>
