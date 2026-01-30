<script lang="ts">
	import type { ExchangeStatus, ExchangeId } from '$lib/types';
	import Card from '$lib/components/ui/Card.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';

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
		online: { color: 'profit' as const, label: 'Online', icon: '●' },
		offline: { color: 'loss' as const, label: 'Offline', icon: '○' },
		degraded: { color: 'warning' as const, label: 'Degraded', icon: '◐' }
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
</script>

<Card variant="elevated" padding="medium" class="exchange-card">
	<!-- Header -->
	<div class="flex items-center justify-between mb-4">
		<div class="flex items-center gap-3">
			<div 
				class="exchange-icon"
				style="background-color: {exchangeColors[status.exchange]}20; color: {exchangeColors[status.exchange]}"
			>
				<span class="text-title-sm font-bold">{exchangeNames[status.exchange][0]}</span>
			</div>
			<div>
				<h3 class="text-title-md font-semibold text-on-surface">{exchangeNames[status.exchange]}</h3>
				<span class="text-label-sm text-on-surface-variant">{status.exchange.toUpperCase()}</span>
			</div>
		</div>
		<Badge variant="tonal" color={statusConfig[status.status].color} size="small">
			<span class="flex items-center gap-1">
				<span class="status-dot" style="color: {status.status === 'online' ? 'var(--color-profit)' : status.status === 'degraded' ? 'var(--color-warning)' : 'var(--color-loss)'}" class:pulse={status.status === 'online'}>
					{statusConfig[status.status].icon}
				</span>
				{statusConfig[status.status].label}
			</span>
		</Badge>
	</div>
	
	<!-- Metrics Grid -->
	<div class="metrics-grid">
		<div class="metric-item">
			<span class="metric-label">Latency</span>
			<div class="flex items-center gap-2">
				<span class="metric-value font-mono {latencyColor() === 'profit' ? 'text-profit' : latencyColor() === 'warning' ? 'text-warning' : 'text-loss'}">
					{status.latency}ms
				</span>
				{#if status.latency < 100}
					<svg class="w-4 h-4 text-profit" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M5 12h14M12 5l7 7-7 7" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				{/if}
			</div>
		</div>
		
		<div class="metric-item">
			<span class="metric-label">Last Update</span>
			<span class="metric-value font-mono">{timeSinceUpdate()} ago</span>
		</div>
	</div>
	
	<!-- Rate Limit Progress -->
	<div class="mt-4">
		<div class="flex items-center justify-between mb-2">
			<span class="metric-label">Rate Limit</span>
			<span class="text-label-sm font-mono {rateLimitColor() === 'profit' ? 'text-profit' : rateLimitColor() === 'warning' ? 'text-warning' : 'text-loss'}">
				{Math.round(status.rateLimitUsage * 100)}%
			</span>
		</div>
		<div class="rate-limit-bar">
			<div 
				class="rate-limit-fill {status.rateLimitUsage > 0.8 ? 'bg-loss' : status.rateLimitUsage > 0.6 ? 'bg-warning' : 'bg-profit'}"
				style="width: {status.rateLimitUsage * 100}%"
			></div>
		</div>
	</div>
	
	<!-- Connection Quality Indicator -->
	<div class="mt-4 pt-4 border-t border-outline-variant">
		<div class="flex items-center justify-between">
			<span class="text-label-sm text-on-surface-variant">Connection Quality</span>
			<div class="quality-dots">
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
</Card>

<style>
	:global(.exchange-card) {
		position: relative;
	}
	
	.exchange-icon {
		width: 44px;
		height: 44px;
		border-radius: var(--shape-medium);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	
	.status-dot {
		font-size: 8px;
	}
	
	.status-dot.pulse {
		animation: pulse-live 2s ease-in-out infinite;
	}
	
	.metrics-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 12px;
	}
	
	.metric-item {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	
	.metric-label {
		font: var(--typography-label-sm);
		color: var(--md-sys-color-on-surface-variant);
	}
	
	.metric-value {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
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
	
	.quality-dots {
		display: flex;
		gap: 4px;
	}
	
	.quality-dot {
		width: 6px;
		height: 6px;
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
	
	:global(.text-on-surface) {
		color: var(--md-sys-color-on-surface);
	}
	
	:global(.text-on-surface-variant) {
		color: var(--md-sys-color-on-surface-variant);
	}
	
	:global(.border-outline-variant) {
		border-color: var(--md-sys-color-outline-variant);
	}
	
	:global(.font-mono) {
		font-family: var(--font-mono);
	}
	
	:global(.text-profit) {
		color: var(--color-profit);
	}
	
	:global(.text-loss) {
		color: var(--color-loss);
	}
	
	:global(.text-warning) {
		color: var(--color-warning);
	}
	
	:global(.bg-profit) {
		background-color: var(--color-profit);
	}
	
	:global(.bg-loss) {
		background-color: var(--color-loss);
	}
	
	:global(.bg-warning) {
		background-color: var(--color-warning);
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
