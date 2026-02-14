<script lang="ts">
	import type { ExchangeStatus, ExchangeId } from '$lib/types';
	import Badge from '$lib/components/ui/Badge.svelte';
	import { t } from '$lib/i18n.svelte';

	interface Props {
		status: ExchangeStatus;
	}

	let { status }: Props = $props();

	const exchangeColors: Record<ExchangeId, string> = {
		okx: '#0052FF',
		bybit: '#F7A600',
		mexc: '#1A5AFF',
		gateio: '#3CB371',
		kraken: '#5741D9',
		bitstamp: '#00A6C2'
	};
	
	const exchangeNames: Record<ExchangeId, string> = {
		okx: 'OKX',
		bybit: 'Bybit',
		mexc: 'MEXC',
		gateio: 'Gate.io',
		kraken: 'Kraken',
		bitstamp: 'Bitstamp'
	};
	
	const statusConfig = {
		online: { color: 'profit' as const, labelKey: 'exchangeCard.online', icon: '●' },
		offline: { color: 'loss' as const, labelKey: 'exchangeCard.offline', icon: '○' },
		degraded: { color: 'warning' as const, labelKey: 'exchangeCard.degraded', icon: '◐' }
	};
	
	const timeSinceUpdate = $derived(() => {
		const seconds = Math.floor((Date.now() - status.lastUpdate) / 1000);
		if (seconds < 60) return `${seconds}s`;
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes}m`;
		const hours = Math.floor(minutes / 60);
		return `${hours}h`;
	});
	
	const rateLimitColor = $derived(() => {
		if (status.rateLimitUsage > 0.8) return 'loss' as const;
		if (status.rateLimitUsage > 0.6) return 'warning' as const;
		return 'profit' as const;
	});
	
	const latencyColor = $derived(() => {
		if (status.latency > 500) return 'loss' as const;
		if (status.latency > 200) return 'warning' as const;
		return 'profit' as const;
	});

	const latencyIcon = $derived(() => {
		if (status.latency < 100) return 'fast';
		if (status.latency < 300) return 'normal';
		return 'slow';
	});
</script>

<div class="exchange-card glass" role="region" aria-label="{exchangeNames[status.exchange]} exchange status">
	<!-- Header -->
	<div class="card-header">
		<div class="exchange-info">
			<div 
				class="exchange-icon"
				style="--exchange-color: {exchangeColors[status.exchange]}"
			>
				<span class="exchange-initial">{exchangeNames[status.exchange][0]}</span>
			</div>
			<div class="exchange-details">
				<h3 class="exchange-name">{exchangeNames[status.exchange]}</h3>
				<span class="exchange-id">{status.exchange.toUpperCase()}</span>
			</div>
		</div>
		<Badge variant="tonal" color={statusConfig[status.status].color} size="small">
			<span class="badge-content">
				<span 
					class="status-dot"
					class:pulse={status.status === 'online'}
					class:online={status.status === 'online'}
					class:degraded={status.status === 'degraded'}
					class:offline={status.status === 'offline'}
				></span>
				{t(statusConfig[status.status].labelKey)}
			</span>
		</Badge>
	</div>
	
	<!-- Metrics Grid -->
	<div class="metrics-grid">
		<div class="metric-item">
			<span class="metric-label">{t('exchangeCard.latency')}</span>
			<div class="metric-value-row">
				<span class="metric-value font-mono {latencyColor()}">{status.latency}ms</span>
				{#if latencyIcon() === 'fast'}
					<svg class="metric-icon fast" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
						<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				{:else if latencyIcon() === 'normal'}
					<svg class="metric-icon normal" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
						<path d="M5 12h14" stroke-linecap="round"/>
					</svg>
				{:else}
					<svg class="metric-icon slow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
						<path d="M13 17l5-5-5-5M6 17l5-5-5-5" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				{/if}
			</div>
		</div>
		
		<div class="metric-item">
			<span class="metric-label">{t('exchangeCard.lastUpdate')}</span>
			<span class="metric-value font-mono">{timeSinceUpdate()}</span>
		</div>
	</div>
	
	<!-- Rate Limit Progress -->
	<div class="rate-limit-section">
		<div class="rate-limit-header">
			<span class="metric-label">{t('exchangeCard.rateLimit')}</span>
			<span class="rate-limit-value font-mono {rateLimitColor()}">{Math.round(status.rateLimitUsage * 100)}%</span>
		</div>
		<div class="rate-limit-bar">
			<div 
				class="rate-limit-fill {rateLimitColor()}"
				style="width: {status.rateLimitUsage * 100}%"
			></div>
		</div>
	</div>
	
	<!-- Connection Quality -->
	<div class="quality-section">
		<span class="metric-label">{t('exchangeCard.connectionQuality')}</span>
		<div class="quality-dots" aria-label="Connection quality rating">
			{#each Array(5) as _, i}
				<div 
					class="quality-dot"
					class:active={i < (status.status === 'online' ? 5 : status.status === 'degraded' ? 3 : 1)}
					class:good={status.status === 'online'}
					class:warning={status.status === 'degraded'}
					class:poor={status.status === 'offline'}
				></div>
			{/each}
		</div>
	</div>
</div>

<style>
	.exchange-card {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-4);
		border-radius: var(--shape-extra-large);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
		transition: border-color var(--duration-normal) var(--ease-standard), box-shadow var(--duration-normal) var(--ease-standard);
	}

	.exchange-card:hover {
		border-color: var(--glass-border-strong);
		box-shadow: var(--glass-elevation-2);
	}

	.card-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
	}

	.exchange-info {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}

	.exchange-icon {
		width: 44px;
		height: 44px;
		border-radius: var(--shape-medium);
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--exchange-color) 15%, transparent);
	}

	.exchange-initial {
		font: var(--typography-title-small);
		font-weight: 700;
		color: var(--exchange-color);
	}

	.exchange-details {
		display: flex;
		flex-direction: column;
	}

	.exchange-name {
		font: var(--typography-title-small);
		font-weight: 600;
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.exchange-id {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
	}

	.badge-content {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}

	.status-dot {
		width: 8px;
		height: 8px;
		border-radius: var(--shape-full);
	}

	.status-dot.online {
		background: var(--color-profit);
	}

	.status-dot.degraded {
		background: var(--color-warning);
	}

	.status-dot.offline {
		background: var(--color-loss);
	}

	.status-dot.pulse {
		animation: pulse-live 2s ease-in-out infinite;
	}

	.metrics-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--space-3);
	}

	.metric-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.metric-label {
		font: var(--typography-label-sm);
		color: var(--md-sys-color-on-surface-variant);
	}

	.metric-value-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.metric-value {
		font: var(--typography-body-medium);
		font-family: var(--font-mono);
		color: var(--md-sys-color-on-surface);
	}

	.metric-icon {
		width: 16px;
		height: 16px;
	}

	.metric-icon.fast {
		color: var(--color-profit);
	}

	.metric-icon.normal {
		color: var(--color-warning);
	}

	.metric-icon.slow {
		color: var(--color-loss);
	}

	.rate-limit-section {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.rate-limit-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.rate-limit-value {
		font: var(--typography-label-medium);
		font-family: var(--font-mono);
	}

	.rate-limit-bar {
		height: 6px;
		background-color: var(--md-sys-color-surface-container-high);
		border-radius: var(--shape-full);
		overflow: hidden;
	}

	.rate-limit-fill {
		height: 100%;
		border-radius: var(--shape-full);
		transition: width var(--duration-slow) var(--ease-standard);
	}

	.rate-limit-fill.profit {
		background: var(--color-profit);
	}

	.rate-limit-fill.warning {
		background: var(--color-warning);
	}

	.rate-limit-fill.loss {
		background: var(--color-loss);
	}

	.quality-section {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-top: var(--space-3);
		border-top: 1px solid var(--glass-border);
	}

	.quality-dots {
		display: flex;
		gap: var(--space-1);
	}

	.quality-dot {
		width: 8px;
		height: 8px;
		border-radius: var(--shape-full);
		background-color: var(--md-sys-color-outline-variant);
		transition: background-color var(--duration-fast) var(--ease-standard);
	}

	.quality-dot.active.good {
		background-color: var(--color-profit);
	}

	.quality-dot.active.warning {
		background-color: var(--color-warning);
	}

	.quality-dot.active.poor {
		background-color: var(--color-loss);
	}

	.profit {
		color: var(--color-profit);
	}

	.loss {
		color: var(--color-loss);
	}

	.warning {
		color: var(--color-warning);
	}

	@keyframes pulse-live {
		0%, 100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}
</style>
