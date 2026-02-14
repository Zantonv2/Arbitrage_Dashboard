<script lang="ts">
	import type { TradeSignal, ExchangeId } from '$lib/types';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import { formatCurrency, t, getStrategyName } from '$lib/i18n.svelte';
	import { apiClient } from '$lib/api/client';
	import { toastStore } from '$lib/stores/toast.svelte';

	interface Props {
		signal: TradeSignal;
		variant?: 'compact' | 'detailed';
		onSelect?: (signal: TradeSignal) => void;
		onExecute?: (signal: TradeSignal) => void;
	}

	let { signal, variant = 'compact', onSelect, onExecute }: Props = $props();

	// Execution state
	let isExecuting = $state(false);
	let executionStatus = $state<'idle' | 'preparing' | 'confirming' | 'done'>('idle');

	// Default position size for profit calculation
	const DEFAULT_POSITION_SIZE = 10000;

	const isProfitable = $derived(signal.profitBps > 0);
	const profitColor = $derived(isProfitable ? 'profit' : 'loss');
	const confidencePercent = $derived(Math.round(signal.confidence * 100));
	const actualProfit = $derived((signal.profitBps * DEFAULT_POSITION_SIZE) / 100);
	
	const exchangeColors: Record<ExchangeId, string> = {
		okx: '#0052FF',
		bybit: '#F7A600',
		mexc: '#1A5AFF',
		gateio: '#3CB371',
		kraken: '#5741D9',
		bitstamp: '#00A6C2'
	};
	
	function formatProfit(profit: number): string {
		return formatCurrency(profit);
	}
	
	function formatTime(timestamp: number): string {
		const seconds = Math.floor((Date.now() - timestamp) / 1000);
		if (seconds < 60) return `${seconds}s ago`;
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes}m ago`;
		const hours = Math.floor(minutes / 60);
		return `${hours}h ago`;
	}

	async function handleExecute(e: MouseEvent) {
		e.stopPropagation();
		
		if (isExecuting) return;
		
		isExecuting = true;
		executionStatus = 'preparing';
		
		try {
			// Step 1: Prepare execution
			const prepareResult = await apiClient.prepareExecution(signal.id);
			
			if (!prepareResult.is_valid) {
				toastStore.error(
					'Execution Rejected',
					prepareResult.validation_errors.join(', ')
				);
				executionStatus = 'idle';
				isExecuting = false;
				return;
			}
			
			executionStatus = 'confirming';
			
			// Step 2: Confirm execution
			const confirmResult = await apiClient.confirmExecution(prepareResult.instruction_id);
			
			if (confirmResult.success) {
				toastStore.success(
					'Trade Executed',
					`Profit: ${formatCurrency(confirmResult.actual_profit ?? 0)} in ${confirmResult.execution_time_ms}ms`
				);
				executionStatus = 'done';
				
				// Notify parent
				onExecute?.(signal);
			} else {
				toastStore.error(
					'Execution Failed',
					confirmResult.message
				);
				executionStatus = 'idle';
			}
		} catch (error) {
			toastStore.error(
				'Execution Error',
				error instanceof Error ? error.message : 'Unknown error occurred'
			);
			executionStatus = 'idle';
		} finally {
			isExecuting = false;
		}
	}
</script>

<div 
	class="signal-card glass {isProfitable ? 'signal-profitable' : 'signal-loss'}"
	onclick={() => onSelect?.(signal)}
	onkeydown={(e) => e.key === 'Enter' && onSelect?.(signal)}
	role="button"
	tabindex="0"
	aria-label="{signal.symbol} - {isProfitable ? 'Profit' : 'Loss'}: {formatProfit(actualProfit)}"
>
	<!-- Profit Header -->
	<div class="signal-header">
		<Badge variant="filled" color={profitColor} size="medium">
			{formatProfit(actualProfit)}
		</Badge>
		<div class="signal-meta">
			<span class="confidence">{confidencePercent}%</span>
			<span class="timestamp">{formatTime(signal.timestamp)}</span>
		</div>
	</div>
	
	<!-- Exchange Flow -->
	<div class="exchange-flow">
		<div 
			class="exchange-node"
			style="--exchange-color: {exchangeColors[signal.buyExchange]}"
		>
			<span class="exchange-name">{signal.buyExchange.toUpperCase()}</span>
			<span class="exchange-label">{t('signalCard.buy')}</span>
		</div>
		
		<div class="flow-connector">
			<div class="flow-line"></div>
			<svg class="flow-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
				<path d="M5 12h14M12 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
		</div>
		
		<div 
			class="exchange-node"
			style="--exchange-color: {exchangeColors[signal.sellExchange]}"
		>
			<span class="exchange-name">{signal.sellExchange.toUpperCase()}</span>
			<span class="exchange-label">{t('signalCard.sell')}</span>
		</div>
	</div>
	
	<!-- Symbol & Strategy -->
	<div class="signal-footer">
		<span class="symbol">{signal.symbol}</span>
		<Badge variant="tonal" color="default" size="small">
			{getStrategyName(signal.strategy, true)}
		</Badge>
	</div>
	
	<!-- Detailed View -->
	{#if variant === 'detailed'}
		<div class="details-panel glass">
			<div class="details-grid">
				<div class="detail-item">
					<span class="detail-label">{t('signalCard.buyPrice')}</span>
					<span class="detail-value">${signal.buyPrice.toFixed(4)}</span>
				</div>
				<div class="detail-item">
					<span class="detail-label">{t('signalCard.sellPrice')}</span>
					<span class="detail-value">${signal.sellPrice.toFixed(4)}</span>
				</div>
				<div class="detail-item">
					<span class="detail-label">{t('signalCard.volume')}</span>
					<span class="detail-value">{signal.volume.toFixed(4)}</span>
				</div>
				<div class="detail-item">
					<span class="detail-label">{t('signalCard.spread')}</span>
					<span class="detail-value profit">${(signal.sellPrice - signal.buyPrice).toFixed(4)}</span>
				</div>
			</div>
			<div class="action-row">
				<Button 
					variant="filled" 
					size="small" 
					onclick={handleExecute}
					disabled={isExecuting}
				>
					{#if isExecuting}
						<svg class="btn-icon spinning" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
							<path d="M12 2v4m0 12v4m-8-10H4m16 0h-4m-2.5-6.5L17 4m-10 10l-2.5 2.5M17 17l2.5 2.5M7 7L4.5 4.5" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						{executionStatus === 'preparing' ? t('signalCard.preparing') : t('signalCard.executing')}
					{:else}
						<svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
							<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
						{t('signalCard.execute')}
					{/if}
				</Button>
				<Button variant="tonal" size="small" onclick={(e) => { e.stopPropagation(); onSelect?.(signal); }}>
					<svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
						<path d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
					{t('signalCard.details')}
				</Button>
			</div>
		</div>
	{/if}
</div>

<style>
	.signal-card {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-4);
		border-radius: var(--shape-extra-large);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
		cursor: pointer;
		transition: border-color var(--duration-normal) var(--ease-standard), box-shadow var(--duration-normal) var(--ease-standard), transform var(--duration-normal) var(--ease-standard);
	}

	.signal-card:hover {
		border-color: var(--glass-border-strong);
		box-shadow: var(--glass-elevation-2);
		transform: translateY(-2px);
	}

	.signal-card:focus {
		outline: none;
		box-shadow: 0 0 0 2px var(--md-sys-color-primary);
		z-index: 10;
		position: relative;
	}

	.signal-profitable {
		border-left: 3px solid var(--color-profit);
	}

	.signal-loss {
		border-left: 3px solid var(--color-loss);
	}

	.signal-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
	}

	.signal-meta {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-1);
	}

	.confidence {
		font: var(--typography-label-medium);
		color: var(--md-sys-color-on-surface-variant);
	}

	.timestamp {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
		opacity: 0.7;
	}

	.exchange-flow {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.exchange-node {
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--shape-medium);
		background: color-mix(in srgb, var(--exchange-color) 15%, transparent);
		min-width: 60px;
	}

	.exchange-name {
		font: var(--typography-label-medium);
		font-weight: 600;
		color: var(--exchange-color);
	}

	.exchange-label {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
		opacity: 0.7;
	}

	.flow-connector {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		position: relative;
	}

	.flow-line {
		position: absolute;
		left: 0;
		right: 0;
		height: 2px;
		background: linear-gradient(90deg, var(--md-sys-color-outline-variant), var(--md-sys-color-primary));
	}

	.flow-arrow {
		width: 18px;
		height: 18px;
		color: var(--md-sys-color-primary);
		position: relative;
		z-index: 1;
	}

	.signal-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.symbol {
		font: var(--typography-title-small);
		font-family: var(--font-mono);
		font-weight: 600;
		color: var(--md-sys-color-on-surface);
	}

	.details-panel {
		margin-top: var(--space-2);
		padding: var(--space-4);
		border-radius: var(--shape-large);
		border: 1px solid var(--glass-border);
		background: var(--glass-background-subtle);
	}

	.details-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--space-3);
		margin-bottom: var(--space-4);
	}

	.detail-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.detail-label {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
	}

	.detail-value {
		font: var(--typography-body-medium);
		font-family: var(--font-mono);
		color: var(--md-sys-color-on-surface);
	}

	.detail-value.profit {
		color: var(--color-profit);
	}

	.action-row {
		display: flex;
		gap: var(--space-2);
	}

	.btn-icon {
		width: 16px;
		height: 16px;
	}

	.btn-icon.spinning {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
</style>
