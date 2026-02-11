<script lang="ts">
	import type { StrategyType } from '$lib/types';
	import { systemStore } from '$lib/stores/system.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';

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

<div class="strategy-selector glass">
	<div class="selector-header">
		<h2 class="selector-title">Strategies</h2>
		<div class="selector-actions">
			<button class="action-link" onclick={() => toggleAll(true)}>
				Enable All
			</button>
			<span class="action-separator">|</span>
			<button class="action-link" onclick={() => toggleAll(false)}>
				Disable All
			</button>
		</div>
	</div>

	<fieldset class="strategy-list">
		<legend class="sr-only">Select trading strategies</legend>
		{#each allStrategies as strategy}
			{@const isEnabled = systemStore.enabledStrategies.includes(strategy)}
			<label class="strategy-item glass-interactive" class:enabled={isEnabled}>
				<input
					type="checkbox"
					checked={isEnabled}
					onchange={() => systemStore.toggleStrategy(strategy)}
					class="strategy-checkbox"
				/>
				<span class="strategy-label">{strategyLabels[strategy]}</span>
				{#if isEnabled}
					<Badge variant="tonal" color="profit" size="small">Active</Badge>
				{/if}
			</label>
		{/each}
	</fieldset>

	<div class="selector-footer">
		<span class="footer-text">{systemStore.enabledStrategies.length} of {allStrategies.length} strategies enabled</span>
	</div>
</div>

<style>
	.strategy-selector {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-5);
		border-radius: var(--shape-extra-large);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
	}

	.selector-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.selector-title {
		font: var(--typography-title-medium);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.selector-actions {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.action-link {
		background: none;
		border: none;
		color: var(--md-sys-color-primary);
		font: var(--typography-label-medium);
		cursor: pointer;
		padding: var(--space-1) var(--space-2);
		border-radius: var(--shape-small);
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.action-link:hover {
		background: var(--md-sys-color-surface-container-high);
	}

	.action-separator {
		color: var(--md-sys-color-outline);
	}

	.strategy-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		border: none;
		padding: 0;
		margin: 0;
	}

	.strategy-item {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--shape-large);
		border: 1px solid var(--glass-border);
		background: transparent;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.strategy-item:hover {
		background: var(--glass-background-subtle);
		border-color: var(--glass-border-strong);
	}

	.strategy-item.enabled {
		background: var(--md-sys-color-primary-container);
		border-color: var(--md-sys-color-primary);
	}

	.strategy-checkbox {
		width: 18px;
		height: 18px;
		border-radius: var(--shape-small);
		border: 2px solid var(--md-sys-color-outline);
		background: transparent;
		cursor: pointer;
		accent-color: var(--md-sys-color-primary);
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.strategy-checkbox:checked {
		background: var(--md-sys-color-primary);
		border-color: var(--md-sys-color-primary);
	}

	.strategy-label {
		flex: 1;
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
	}

	.selector-footer {
		padding-top: var(--space-3);
		border-top: 1px solid var(--glass-border);
	}

	.footer-text {
		font: var(--typography-label-medium);
		color: var(--md-sys-color-on-surface-variant);
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
