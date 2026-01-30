<script lang="ts">
	import type { StrategyType } from '$lib/types';
	import { systemStore } from '$lib/stores/system.svelte';

	const allStrategies: StrategyType[] = [
		'cex-arbitrage',
		'funding-arbitrage',
		'spot-perp-arbitrage',
		'stablecoin-arbitrage',
		'cross-exchange-arbitrage',
		'spread-capture',
		'latency-arbitrage',
		'convergence-arbitrage',
		'new-listing-arbitrage',
		'hedged-funding'
	];

	const strategyLabels: Record<StrategyType, string> = {
		'cex-arbitrage': 'CEX Arbitrage',
		'funding-arbitrage': 'Funding Rate Arbitrage',
		'spot-perp-arbitrage': 'Spot-Perp Arbitrage',
		'stablecoin-arbitrage': 'Stablecoin Arbitrage',
		'cross-exchange-arbitrage': 'Cross-Exchange Arbitrage',
		'spread-capture': 'Spread Capture',
		'latency-arbitrage': 'Latency Arbitrage',
		'convergence-arbitrage': 'Convergence Arbitrage',
		'new-listing-arbitrage': 'New Listing Arbitrage',
		'hedged-funding': 'Hedged Funding'
	};

	function toggleAll(enable: boolean): void {
		if (enable) {
			systemStore.enableAllStrategies(allStrategies);
		} else {
			systemStore.disableAllStrategies();
		}
	}

	const allEnabled = $derived(systemStore.enabledStrategies.length === allStrategies.length);
</script>

<div class="strategy-selector bg-white dark:bg-gray-800 rounded-lg p-6 border border-gray-200 dark:border-gray-700">
	<div class="flex items-center justify-between mb-4">
		<h2 class="text-xl font-semibold">Strategies</h2>
		<div class="flex gap-2">
			<button
				class="text-sm text-blue-600 dark:text-blue-400 hover:underline"
				onclick={() => toggleAll(true)}
			>
				Enable All
			</button>
			<span class="text-gray-400">|</span>
			<button
				class="text-sm text-blue-600 dark:text-blue-400 hover:underline"
				onclick={() => toggleAll(false)}
			>
				Disable All
			</button>
		</div>
	</div>

	<fieldset class="space-y-3">
		<legend class="sr-only">Select trading strategies</legend>
		{#each allStrategies as strategy}
			{@const isEnabled = systemStore.enabledStrategies.includes(strategy)}
			<label class="flex items-center gap-3 p-3 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700/50 cursor-pointer transition-colors">
				<input
					type="checkbox"
					checked={isEnabled}
					onchange={() => systemStore.toggleStrategy(strategy)}
					class="w-5 h-5 rounded border-gray-300 dark:border-gray-600 text-blue-600 focus:ring-2 focus:ring-blue-500"
				/>
				<div class="flex-1">
					<div class="font-medium">{strategyLabels[strategy]}</div>
				</div>
			</label>
		{/each}
	</fieldset>

	<div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 text-sm text-gray-600 dark:text-gray-400">
		{systemStore.enabledStrategies.length} of {allStrategies.length} strategies enabled
	</div>
</div>
