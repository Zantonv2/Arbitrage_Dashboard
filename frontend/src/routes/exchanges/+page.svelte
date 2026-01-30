<script lang="ts">
	import ExchangeCard from '$lib/components/ExchangeCard.svelte';
	import { exchangesStore } from '$lib/stores/exchanges.svelte';
</script>

<svelte:head>
	<title>Exchanges - Arbitrage Dashboard</title>
</svelte:head>

<div class="exchanges-page">
	<h1 class="text-3xl font-bold mb-6">Exchange Status</h1>

	<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
		{#each exchangesStore.allExchanges as exchange (exchange.exchange)}
			<ExchangeCard status={exchange} />
		{:else}
			<div class="col-span-full text-center text-gray-500 dark:text-gray-400 py-12">
				No exchange data available
			</div>
		{/each}
	</div>

	<div class="mt-8 grid grid-cols-1 md:grid-cols-3 gap-6">
		<div class="bg-white dark:bg-gray-800 rounded-lg p-6 border border-gray-200 dark:border-gray-700">
			<h3 class="text-lg font-semibold mb-2">Online</h3>
			<div class="text-3xl font-bold text-green-600 dark:text-green-400">
				{exchangesStore.onlineExchanges.length}
			</div>
		</div>

		<div class="bg-white dark:bg-gray-800 rounded-lg p-6 border border-gray-200 dark:border-gray-700">
			<h3 class="text-lg font-semibold mb-2">Offline</h3>
			<div class="text-3xl font-bold text-red-600 dark:text-red-400">
				{exchangesStore.offlineExchanges.length}
			</div>
		</div>

		<div class="bg-white dark:bg-gray-800 rounded-lg p-6 border border-gray-200 dark:border-gray-700">
			<h3 class="text-lg font-semibold mb-2">Total</h3>
			<div class="text-3xl font-bold text-blue-600 dark:text-blue-400">
				{exchangesStore.allExchanges.length}
			</div>
		</div>
	</div>
</div>
