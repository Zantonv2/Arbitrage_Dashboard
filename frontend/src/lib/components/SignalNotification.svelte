<script lang="ts">
	import type { TradeSignal } from '$lib/types';
	import Button from '$lib/components/ui/Button.svelte';

	interface Props {
		signal: TradeSignal;
		onExecute?: (signal: TradeSignal) => void;
		onDismiss?: () => void;
	}

	let { signal, onExecute, onDismiss }: Props = $props();

	const exchangeColors: Record<string, string> = {
		okx: '#0052FF',
		bybit: '#F7A600',
		mexc: '#1A5AFF',
		gateio: '#3CB371',
		kraken: '#5741D9',
		bitstamp: '#00A6C2'
	};

	const isProfitable = $derived(signal.profitBps > 0);
	const profitColor = $derived(isProfitable ? 'var(--color-profit)' : 'var(--color-loss)');

	function formatProfit(bps: number): string {
		return `${bps > 0 ? '+' : ''}${bps.toFixed(2)} bps`;
	}

	function formatPrice(price: number): string {
		return price.toLocaleString('en-US', {
			minimumFractionDigits: 2,
			maximumFractionDigits: 6
		});
	}

	function handleExecute() {
		onExecute?.(signal);
	}
</script>

<div class="signal-notification glass glass-elevation-2">
	<div class="notification-header">
		<div class="signal-badge" style="background-color: {profitColor}20; color: {profitColor}">
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			<span>{formatProfit(signal.profitBps)}</span>
		</div>
		<button class="dismiss-btn" onclick={onDismiss} aria-label="Dismiss notification">
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
		</button>
	</div>

	<div class="signal-flow">
		<div class="exchange-badge" style="background-color: {exchangeColors[signal.buyExchange]}20; color: {exchangeColors[signal.buyExchange]}">
			<span class="exchange-name">{signal.buyExchange.toUpperCase()}</span>
			<span class="exchange-action">Buy</span>
			<span class="exchange-price">{formatPrice(signal.buyPrice)}</span>
		</div>
		
		<div class="flow-arrow">
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M5 12h14M12 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
		</div>
		
		<div class="exchange-badge" style="background-color: {exchangeColors[signal.sellExchange]}20; color: {exchangeColors[signal.sellExchange]}">
			<span class="exchange-name">{signal.sellExchange.toUpperCase()}</span>
			<span class="exchange-action">Sell</span>
			<span class="exchange-price">{formatPrice(signal.sellPrice)}</span>
		</div>
	</div>

	<div class="signal-details">
		<div class="detail-row">
			<span class="detail-label">Symbol</span>
			<span class="detail-value">{signal.symbol}</span>
		</div>
		<div class="detail-row">
			<span class="detail-label">Strategy</span>
			<span class="detail-value">{signal.strategy.replace(/-/g, ' ').replace(/\b\w/g, c => c.toUpperCase())}</span>
		</div>
		<div class="detail-row">
			<span class="detail-label">Volume</span>
			<span class="detail-value">{signal.volume.toFixed(4)}</span>
		</div>
		<div class="detail-row">
			<span class="detail-label">Confidence</span>
			<span class="detail-value">{Math.round(signal.confidence * 100)}%</span>
		</div>
	</div>

	<div class="notification-actions">
		<Button variant="text" size="small" onclick={onDismiss}>
			Dismiss
		</Button>
		<Button variant="filled" size="small" onclick={handleExecute}>
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width: 16px; height: 16px;">
				<path d="M5 12h14M12 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			Execute Trade
		</Button>
	</div>
</div>

<style>
	.signal-notification {
		padding: var(--space-4);
		border-radius: var(--shape-large);
		border: 1px solid var(--glass-border);
		animation: slide-in var(--duration-normal) var(--ease-emphasized-decelerate);
	}

	@keyframes slide-in {
		from {
			opacity: 0;
			transform: translateX(20px);
		}
		to {
			opacity: 1;
			transform: translateX(0);
		}
	}

	.notification-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--space-4);
	}

	.signal-badge {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--shape-full);
		font: var(--typography-label-large);
		font-weight: 600;
	}

	.signal-badge svg {
		width: 18px;
		height: 18px;
	}

	.dismiss-btn {
		width: 32px;
		height: 32px;
		border-radius: var(--shape-full);
		border: none;
		background: transparent;
		color: var(--md-sys-color-on-surface-variant);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.dismiss-btn:hover {
		background: var(--md-sys-color-surface-container-high);
		color: var(--md-sys-color-on-surface);
	}

	.dismiss-btn svg {
		width: 16px;
		height: 16px;
	}

	.signal-flow {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		margin-bottom: var(--space-4);
	}

	.exchange-badge {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: var(--space-3);
		border-radius: var(--shape-medium);
	}

	.exchange-name {
		font: var(--typography-title-small);
		font-weight: 600;
	}

	.exchange-action {
		font: var(--typography-label-small);
		opacity: 0.7;
		margin-top: var(--space-1);
	}

	.exchange-price {
		font: var(--typography-body-small);
		font-family: var(--font-mono);
		margin-top: var(--space-1);
	}

	.flow-arrow {
		color: var(--md-sys-color-on-surface-variant);
	}

	.flow-arrow svg {
		width: 24px;
		height: 24px;
	}

	.signal-details {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--space-2);
		padding: var(--space-3);
		background: var(--glass-background-subtle);
		border-radius: var(--shape-medium);
		margin-bottom: var(--space-4);
	}

	.detail-row {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.detail-label {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
	}

	.detail-value {
		font: var(--typography-body-small);
		color: var(--md-sys-color-on-surface);
		text-transform: capitalize;
	}

	.notification-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
