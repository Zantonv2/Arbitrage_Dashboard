<script lang="ts">
	import type { TradeSignal, ExchangeId } from '$lib/types';
	import Card from '$lib/components/ui/Card.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Button from '$lib/components/ui/Button.svelte';

	interface Props {
		signal: TradeSignal;
		variant?: 'compact' | 'detailed';
		onSelect?: (signal: TradeSignal) => void;
		onExecute?: (signal: TradeSignal) => void;
	}

	let { signal, variant = 'compact', onSelect, onExecute }: Props = $props();

	const isProfitable = $derived(signal.profitBps > 0);
	const profitColor = $derived(isProfitable ? 'profit' : 'loss');
	const confidencePercent = $derived(Math.round(signal.confidence * 100));
	
	const exchangeColors: Record<ExchangeId, string> = {
		okx: '#0052FF',
		bybit: '#F7A600',
		mexc: '#1A5AFF',
		gateio: '#3CB371',
		kraken: '#5741D9',
		bitstamp: '#00A6C2'
	};
	
	const strategyLabels: Record<string, string> = {
		'cex-arbitrage': 'CEX Arb',
		'funding-arbitrage': 'Funding',
		'spot-perp-arbitrage': 'Spot-Perp',
		'stablecoin-arbitrage': 'Stable',
		'cross-exchange-arbitrage': 'Cross-Ex',
		'spread-capture': 'Spread',
		'latency-arbitrage': 'Latency',
		'convergence-arbitrage': 'Convergence',
		'new-listing-arbitrage': 'New List',
		'hedged-funding': 'Hedged'
	};
	
	function formatProfit(bps: number): string {
		return `${bps > 0 ? '+' : ''}${bps.toFixed(2)} bps`;
	}
	
	function formatTime(timestamp: number): string {
		const seconds = Math.floor((Date.now() - timestamp) / 1000);
		if (seconds < 60) return `${seconds}s ago`;
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes}m ago`;
		const hours = Math.floor(minutes / 60);
		return `${hours}h ago`;
	}
</script>

<Card 
	variant="elevated" 
	padding={variant === 'detailed' ? 'large' : 'medium'}
	interactive={!!onSelect}
	onclick={() => onSelect?.(signal)}
	class="signal-card group {isProfitable ? 'signal-profitable' : 'signal-loss'}"
>
	<!-- Header: Profit Badge & Confidence -->
	<div class="flex items-start justify-between mb-3">
		<Badge variant="filled" color={profitColor} size="medium">
			{formatProfit(signal.profitBps)}
		</Badge>
		<div class="flex flex-col items-end gap-1">
			<span class="text-label-sm text-on-surface-variant">{confidencePercent}% confidence</span>
			<span class="text-label-sm text-on-surface-variant">{formatTime(signal.timestamp)}</span>
		</div>
	</div>
	
	<!-- Exchange Flow -->
	<div class="flex items-center gap-3 my-4">
		<div 
			class="exchange-badge"
			style="background-color: {exchangeColors[signal.buyExchange]}20; color: {exchangeColors[signal.buyExchange]}"
		>
			<span class="text-label-md font-semibold">{signal.buyExchange.toUpperCase()}</span>
			<span class="text-label-sm opacity-80">Buy</span>
		</div>
		
		<div class="flex-1 flex items-center justify-center">
			<div class="flow-line"></div>
			<svg class="flow-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M5 12h14M12 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
		</div>
		
		<div 
			class="exchange-badge"
			style="background-color: {exchangeColors[signal.sellExchange]}20; color: {exchangeColors[signal.sellExchange]}"
		>
			<span class="text-label-md font-semibold">{signal.sellExchange.toUpperCase()}</span>
			<span class="text-label-sm opacity-80">Sell</span>
		</div>
	</div>
	
	<!-- Symbol & Strategy -->
	<div class="flex items-center justify-between mb-3">
		<div class="symbol-display">
			<span class="text-title-md font-mono font-semibold text-on-surface">{signal.symbol}</span>
		</div>
		<Badge variant="outlined" color="default" size="small">
			{strategyLabels[signal.strategy] || signal.strategy}
		</Badge>
	</div>
	
	<!-- Detailed View Additional Info -->
	{#if variant === 'detailed'}
		<div class="details-grid">
			<div class="detail-item">
				<span class="detail-label">Buy Price</span>
				<span class="detail-value font-mono">${signal.buyPrice.toFixed(4)}</span>
			</div>
			<div class="detail-item">
				<span class="detail-label">Sell Price</span>
				<span class="detail-value font-mono">${signal.sellPrice.toFixed(4)}</span>
			</div>
			<div class="detail-item">
				<span class="detail-label">Volume</span>
				<span class="detail-value font-mono">{signal.volume.toFixed(4)}</span>
			</div>
			<div class="detail-item">
				<span class="detail-label">Spread</span>
				<span class="detail-value font-mono text-profit">${(signal.sellPrice - signal.buyPrice).toFixed(4)}</span>
			</div>
		</div>
		
		<!-- Action Buttons -->
		<div class="flex gap-3 mt-6 pt-4 border-t border-outline-variant">
			<Button variant="filled" size="medium" onclick={() => onExecute?.(signal)}>
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				Execute
			</Button>
			<Button variant="outlined" size="medium" onclick={() => onSelect?.(signal)}>
				View Details
			</Button>
		</div>
	{/if}
</Card>

<style>
	:global(.signal-card) {
		position: relative;
		overflow: hidden;
	}
	
	:global(.signal-profitable) {
		border-color: rgba(46, 125, 50, 0.3) !important;
	}
	
	:global(.signal-profitable::before) {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 3px;
		background: linear-gradient(90deg, var(--color-profit), var(--color-profit-light));
		opacity: 0.6;
	}
	
	:global(.signal-loss) {
		border-color: rgba(198, 40, 40, 0.3) !important;
	}
	
	:global(.signal-loss::before) {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 3px;
		background: linear-gradient(90deg, var(--color-loss), var(--color-loss-light));
		opacity: 0.6;
	}
	
	.exchange-badge {
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 8px 12px;
		border-radius: var(--shape-medium);
		min-width: 70px;
	}
	
	.flow-line {
		flex: 1;
		height: 2px;
		background: linear-gradient(90deg, var(--md-sys-color-outline-variant), var(--md-sys-color-primary));
		margin: 0 -8px;
	}
	
	.flow-arrow {
		width: 20px;
		height: 20px;
		color: var(--md-sys-color-primary);
		flex-shrink: 0;
	}
	
	.symbol-display {
		position: relative;
	}
	
	.details-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 16px;
		margin-top: 16px;
		padding-top: 16px;
		border-top: 1px solid var(--md-sys-color-outline-variant);
	}
	
	.detail-item {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	
	.detail-label {
		font: var(--typography-label-sm);
		color: var(--md-sys-color-on-surface-variant);
	}
	
	.detail-value {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
	}
	
	:global(.border-outline-variant) {
		border-color: var(--md-sys-color-outline-variant);
	}
	
	:global(.text-on-surface-variant) {
		color: var(--md-sys-color-on-surface-variant);
	}
	
	:global(.text-on-surface) {
		color: var(--md-sys-color-on-surface);
	}
	
	:global(.font-mono) {
		font-family: var(--font-mono);
	}
	
	:global(.text-profit) {
		color: var(--color-profit);
	}
</style>
