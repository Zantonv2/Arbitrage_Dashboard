<script lang="ts">
	import StrategySelector from '$lib/components/StrategySelector.svelte';
	import ExecutionModeToggle from '$lib/components/ExecutionModeToggle.svelte';
</script>

<svelte:head>
	<title>Strategies - Arbitrage Dashboard</title>
</svelte:head>

<div class="strategies-page">
	<h1 class="text-3xl font-bold mb-6">Strategy Configuration</h1>

	<div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
		<StrategySelector />
		<ExecutionModeToggle />
	</div>

	<div class="mt-8 bg-white dark:bg-gray-800 rounded-lg p-6 border border-gray-200 dark:border-gray-700">
		<h2 class="text-xl font-semibold mb-4">Strategy Information</h2>
		<div class="space-y-4 text-sm text-gray-600 dark:text-gray-400">
			<p>
				<strong class="text-gray-900 dark:text-gray-100">CEX Arbitrage:</strong> Exploits price differences between centralized exchanges for the same asset.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">Funding Rate Arbitrage:</strong> Profits from funding rate differences in perpetual futures markets.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">Spot-Perp Arbitrage:</strong> Captures price discrepancies between spot and perpetual futures markets.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">Stablecoin Arbitrage:</strong> Exploits deviations from peg in stablecoin prices.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">Cross-Exchange Arbitrage:</strong> Trades across multiple exchanges to capture price inefficiencies.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">Spread Capture:</strong> Profits from bid-ask spread opportunities.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">Latency Arbitrage:</strong> Uses speed advantages to capture fleeting price differences.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">Convergence Arbitrage:</strong> Bets on price convergence between related assets.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">New Listing Arbitrage:</strong> Exploits price volatility during new token listings.
			</p>
			<p>
				<strong class="text-gray-900 dark:text-gray-100">Hedged Funding:</strong> Captures funding rates while maintaining market-neutral positions.
			</p>
		</div>
	</div>
</div>
