<script lang="ts">
	import Button from '$lib/components/ui/Button.svelte';
	import { systemStore } from '$lib/stores/system.svelte';
	import { setLanguage, getLanguage } from '$lib/i18n.svelte';
	import type { ExchangeId, StrategyType, Language } from '$lib/types';

	// Available exchanges
	const exchanges: { id: ExchangeId; name: string; logo: string }[] = [
		{ id: 'okx', name: 'OKX', logo: 'O' },
		{ id: 'bybit', name: 'Bybit', logo: 'B' },
		{ id: 'mexc', name: 'MEXC', logo: 'M' },
		{ id: 'gateio', name: 'Gate.io', logo: 'G' },
		{ id: 'kraken', name: 'Kraken', logo: 'K' },
		{ id: 'bitstamp', name: 'Bitstamp', logo: 'S' }
	];

	// Available strategies with detailed descriptions
	const strategies: { id: StrategyType; name: string; description: string; detailedInfo: string }[] = [
		{ 
			id: 'cex-arbitrage', 
			name: 'CEX Arbitrage', 
			description: 'Profit from price differences between centralized exchanges',
			detailedInfo: 'Exploits price differences between centralized exchanges for the same asset. Monitors multiple CEX order books and executes simultaneous buy/sell orders when profitable opportunities exceed fee thresholds.'
		},
		{ 
			id: 'funding-arbitrage', 
			name: 'Funding Rate', 
			description: 'Capture funding rate differentials across perpetual markets',
			detailedInfo: 'Profits from funding rate differences in perpetual futures markets. Long funding on exchanges with positive rates, short on those with negative rates to capture the rate differential.'
		},
		{ 
			id: 'spot-perp-arbitrage', 
			name: 'Spot-Perpetual', 
			description: 'Arbitrage between spot and perpetual futures',
			detailedInfo: 'Captures price discrepancies between spot and perpetual futures markets. When perpetual futures trade at a premium to spot, sell the perpetual and buy spot. Reverse when at a discount.'
		},
		{ 
			id: 'stablecoin-arbitrage', 
			name: 'Stablecoin', 
			description: 'Profit from stablecoin price deviations',
			detailedInfo: 'Exploits deviations from peg in stablecoin prices. Monitor USDT, USDC, DAI and other stablecoins across exchanges for temporary depegging opportunities.'
		},
		{ 
			id: 'cross-exchange-arbitrage', 
			name: 'Cross-Exchange', 
			description: 'Multi-exchange arbitrage opportunities',
			detailedInfo: 'Trades across multiple exchanges to capture price inefficiencies. Advanced triangulation between 3+ exchanges for larger opportunities with lower single-point failure risk.'
		},
		{ 
			id: 'latency-arbitrage', 
			name: 'Latency', 
			description: 'Fast execution based arbitrage',
			detailedInfo: 'Uses speed advantages to capture fleeting price differences. Leverages co-location and optimized WebSocket connections to be first in queue for price movements.'
		},
		{ 
			id: 'convergence-arbitrage', 
			name: 'Convergence', 
			description: 'Mean reversion between related assets',
			detailedInfo: 'Bets on price convergence between related assets. When correlated assets diverge beyond historical norms, position for convergence while hedging directional risk.'
		}
	];

	// State - use system store for enabled strategies
	let enabledExchanges = $state<ExchangeId[]>(['okx', 'bybit', 'gateio']);
	let autoExecution = $state(false);
	let confidenceThreshold = $state(0.7);
	let maxPositionSize = $state(1000);
	let stopLoss = $state(5);
	let theme = $state<'light' | 'dark'>('light');
	let language = $state<Language>('en');
	let isSaving = $state(false);
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);
	let showStrategyInfo = $state<StrategyType | null>(null);

	function toggleExchange(exchangeId: ExchangeId): void {
		if (enabledExchanges.includes(exchangeId)) {
			enabledExchanges = enabledExchanges.filter(e => e !== exchangeId);
		} else {
			enabledExchanges = [...enabledExchanges, exchangeId];
		}
	}

	function toggleStrategy(strategyId: StrategyType): void {
		if (systemStore.enabledStrategies.includes(strategyId)) {
			systemStore.disableStrategy(strategyId);
		} else {
			systemStore.enableStrategy(strategyId);
		}
	}

	function enableAllExchanges(): void {
		enabledExchanges = exchanges.map(e => e.id);
	}

	function disableAllExchanges(): void {
		enabledExchanges = [];
	}

	function enableAllStrategies(): void {
		strategies.forEach(s => systemStore.enableStrategy(s.id));
	}

	function disableAllStrategies(): void {
		strategies.forEach(s => systemStore.disableStrategy(s.id));
	}

	async function saveSettings(): Promise<void> {
		isSaving = true;
		message = null;
		
		try {
			// Save to system store
			systemStore.setMode(autoExecution ? 'auto' : 'manual');
			systemStore.setConfidenceThreshold(confidenceThreshold);
			
			// Save theme to localStorage
			localStorage.setItem('triangulum_theme', theme);
			document.documentElement.classList.toggle('dark', theme === 'dark');
			
			// Save language
			setLanguage(language);
			document.documentElement.lang = language;
			
			// Simulate API call for persistence
			await new Promise(resolve => setTimeout(resolve, 500));
			
			// Store in localStorage for persistence
			localStorage.setItem('triangulum_settings', JSON.stringify({
				enabledExchanges,
				autoExecution,
				confidenceThreshold,
				maxPositionSize,
				stopLoss,
				theme,
				language,
				enabledStrategies: systemStore.enabledStrategies
			}));
			
			message = { type: 'success', text: 'Settings saved successfully' };
		} catch (error) {
			console.error('Failed to save settings:', error);
			message = { type: 'error', text: 'Failed to save settings' };
		} finally {
			isSaving = false;
		}
	}

	function formatPositionSize(value: number): string {
		return `$${value.toLocaleString()}`;
	}

	// Load settings from localStorage on mount
	$effect(() => {
		const saved = localStorage.getItem('triangulum_settings');
		if (saved) {
			try {
				const parsed = JSON.parse(saved);
				if (parsed.enabledExchanges) enabledExchanges = parsed.enabledExchanges;
				if (parsed.autoExecution !== undefined) autoExecution = parsed.autoExecution;
				if (parsed.confidenceThreshold) confidenceThreshold = parsed.confidenceThreshold;
				if (parsed.maxPositionSize) maxPositionSize = parsed.maxPositionSize;
				if (parsed.stopLoss) stopLoss = parsed.stopLoss;
				if (parsed.theme) theme = parsed.theme;
				if (parsed.language) language = parsed.language;
				if (parsed.enabledStrategies) {
					systemStore.enabledStrategies = parsed.enabledStrategies;
				}
			} catch (e) {
				console.error('Failed to load settings:', e);
			}
		}
		
		// Load language from i18n system
		language = getLanguage();
	});

	// Apply theme change immediately
	$effect(() => {
		document.documentElement.classList.toggle('dark', theme === 'dark');
	});
</script>

<svelte:head>
	<title>Settings - Triangulum</title>
</svelte:head>

<div class="settings-page">
	<h1 class="page-title">Settings</h1>

	<!-- Message Toast -->
	{#if message}
		<div class="message-toast {message.type}">
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				{#if message.type === 'success'}
					<path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
				{:else}
					<path d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
				{/if}
			</svg>
			{message.text}
		</div>
	{/if}

	<!-- Execution Mode Settings -->
	<section class="settings-section">
		<div class="settings-card glass">
			<div class="card-header">
				<div>
					<h2 class="card-title">Execution Mode</h2>
					<p class="card-description">Configure automatic or manual trading execution</p>
				</div>
			</div>
			
			<div class="execution-options">
				<button 
					class="execution-option glass-interactive"
					class:active={!autoExecution}
					onclick={() => autoExecution = false}
				>
					<div class="option-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M12 11c0 3.517-1.009 6.799-2.753 9.571m-3.44-2.04l.054-.09A13.916 13.916 0 008 11a4 4 0 118 0c0 1.017-.07 2.019-.203 3m-2.118 6.844A21.88 21.88 0 0015.171 17m3.839 1.132c.645-2.266.99-4.659.99-7.132A8 8 0 008 4.07M3 15.364c.64-1.319 1-2.8 1-4.364 0-1.457.39-2.823 1.07-4" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</div>
					<div class="option-content">
						<span class="option-name">Manual</span>
						<span class="option-description">Review each signal before execution</span>
					</div>
					{#if !autoExecution}
						<div class="option-check">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
								<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					{/if}
				</button>
				
				<button 
					class="execution-option glass-interactive"
					class:active={autoExecution}
					onclick={() => autoExecution = true}
				>
					<div class="option-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</div>
					<div class="option-content">
						<span class="option-name">Automatic</span>
						<span class="option-description">Execute trades meeting criteria automatically</span>
					</div>
					{#if autoExecution}
						<div class="option-check">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
								<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					{/if}
				</button>
			</div>

			{#if autoExecution}
				<div class="auto-settings">
					<div class="setting-row">
						<div class="setting-info">
							<span class="setting-label">Confidence Threshold</span>
							<span class="setting-description">Minimum confidence level to auto-execute: {Math.round(confidenceThreshold * 100)}%</span>
						</div>
						<input 
							type="range" 
							min="0.5" 
							max="1" 
							step="0.05" 
							bind:value={confidenceThreshold}
							class="slider"
						/>
					</div>
				</div>
			{/if}
		</div>
	</section>

	<!-- Exchange Connectors -->
	<section class="settings-section">
		<div class="settings-card glass">
			<div class="card-header">
				<div>
					<h2 class="card-title">Exchange Connectors</h2>
					<p class="card-description">Enable or disable exchange connections</p>
				</div>
				<div class="section-actions">
					<button class="text-btn" onclick={enableAllExchanges}>Enable All</button>
					<button class="text-btn" onclick={disableAllExchanges}>Disable All</button>
				</div>
			</div>
			
			<div class="exchange-grid">
				{#each exchanges as exchange (exchange.id)}
					<button 
						class="exchange-item glass-interactive"
						class:enabled={enabledExchanges.includes(exchange.id)}
						onclick={() => toggleExchange(exchange.id)}
					>
						<div class="exchange-logo" class:enabled={enabledExchanges.includes(exchange.id)}>
							{exchange.logo}
						</div>
						<div class="exchange-info">
							<span class="exchange-name">{exchange.name}</span>
							<span class="exchange-status">
								{enabledExchanges.includes(exchange.id) ? 'Enabled' : 'Disabled'}
							</span>
						</div>
						<div class="exchange-check">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
								<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					</button>
				{/each}
			</div>
		</div>
	</section>

	<!-- Strategy Configuration with Info Tooltips -->
	<section class="settings-section">
		<div class="settings-card glass">
			<div class="card-header">
				<div>
					<h2 class="card-title">Strategy Configuration</h2>
					<p class="card-description">Select which trading strategies to enable. Click the info icon for detailed descriptions.</p>
				</div>
				<div class="section-actions">
					<button class="text-btn" onclick={enableAllStrategies}>Enable All</button>
					<button class="text-btn" onclick={disableAllStrategies}>Disable All</button>
				</div>
			</div>
			
			<div class="strategy-grid">
				{#each strategies as strategy (strategy.id)}
					<div 
						class="strategy-item-wrapper"
						onmouseenter={() => showStrategyInfo = strategy.id}
						onmouseleave={() => showStrategyInfo = null}
						role="presentation"
					>
						<button 
							class="strategy-item glass-interactive"
							class:enabled={systemStore.enabledStrategies.includes(strategy.id)}
							onclick={() => toggleStrategy(strategy.id)}
						>
							<div class="strategy-header">
								<span class="strategy-name">{strategy.name}</span>
								<div 
									class="info-trigger"
									onclick={(e) => { e.stopPropagation(); showStrategyInfo = showStrategyInfo === strategy.id ? null : strategy.id; }}
									onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.stopPropagation(); showStrategyInfo = showStrategyInfo === strategy.id ? null : strategy.id; } }}
									tabindex="0"
									role="button"
									aria-label="View strategy information"
								>
									<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<circle cx="12" cy="12" r="10" stroke-linecap="round" stroke-linejoin="round"/>
										<path d="M12 16v-4M12 8h.01" stroke-linecap="round" stroke-linejoin="round"/>
									</svg>
								</div>
							</div>
							<p class="strategy-description">{strategy.description}</p>
							<div class="strategy-check" class:active={systemStore.enabledStrategies.includes(strategy.id)}>
								<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
									<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
							</div>
						</button>
						
						<!-- Info Tooltip -->
						{#if showStrategyInfo === strategy.id}
							<div class="strategy-tooltip glass">
								<h4 class="tooltip-title">{strategy.name}</h4>
								<p class="tooltip-content">{strategy.detailedInfo}</p>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		</div>
	</section>

	<!-- Risk Management -->
	<section class="settings-section">
		<div class="settings-card glass">
			<div class="card-header">
				<h2 class="card-title">Risk Management</h2>
				<p class="card-description">Configure position limits and stop loss</p>
			</div>
			
			<div class="risk-grid">
				<div class="risk-item">
					<label class="risk-label">Max Position Size</label>
					<div class="risk-control">
						<input 
							type="number" 
							bind:value={maxPositionSize}
							min="100"
							max="10000"
							step="100"
							class="glass-input number-input"
						/>
						<span class="risk-unit">USD</span>
					</div>
				</div>

				<div class="risk-item">
					<label class="risk-label">Stop Loss</label>
					<div class="risk-control">
						<input 
							type="number" 
							bind:value={stopLoss}
							min="1"
							max="20"
							class="glass-input number-input"
						/>
						<span class="risk-unit">%</span>
					</div>
				</div>
			</div>
		</div>
	</section>

	<!-- Theme Settings -->
	<section class="settings-section">
		<div class="settings-card glass">
			<div class="card-header">
				<h2 class="card-title">Theme</h2>
				<p class="card-description">Choose light or dark mode</p>
			</div>
			
			<div class="theme-options">
				<button 
					class="theme-option glass-interactive"
					class:active={theme === 'light'}
					onclick={() => theme = 'light'}
				>
					<div class="option-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<circle cx="12" cy="12" r="5" stroke-linecap="round" stroke-linejoin="round"/>
							<path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</div>
					<div class="option-content">
						<span class="option-name">Light Mode</span>
						<span class="option-description">Bright and clean interface</span>
					</div>
					{#if theme === 'light'}
						<div class="option-check">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
								<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					{/if}
				</button>
				
				<button 
					class="theme-option glass-interactive"
					class:active={theme === 'dark'}
					onclick={() => theme = 'dark'}
				>
					<div class="option-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</div>
					<div class="option-content">
						<span class="option-name">Dark Mode</span>
						<span class="option-description">Easy on the eyes</span>
					</div>
					{#if theme === 'dark'}
						<div class="option-check">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
								<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					{/if}
				</button>
			</div>
		</div>
	</section>
	
	<!-- Language Settings -->
	<section class="settings-section">
		<div class="settings-card glass">
			<div class="card-header">
				<h2 class="card-title">Language</h2>
				<p class="card-description">Select your preferred language</p>
			</div>
			
			<div class="language-options">
				<button 
					class="language-option glass-interactive"
					class:active={language === 'en'}
					onclick={() => { language = 'en'; setLanguage('en'); document.documentElement.lang = 'en'; }}
				>
					<div class="option-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M5 13l4 4L19 7M12 3v2M12 21v2M3 12h2M21 12h2" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</div>
					<div class="option-content">
						<span class="option-name">English</span>
						<span class="option-description">English language</span>
					</div>
					{#if language === 'en'}
						<div class="option-check">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
								<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					{/if}
				</button>
				
				<button 
					class="language-option glass-interactive"
					class:active={language === 'ru'}
					onclick={() => { language = 'ru'; setLanguage('ru'); document.documentElement.lang = 'ru'; }}
				>
					<div class="option-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M12 2L2 7l10 5 10-5-10-5z" stroke-linecap="round" stroke-linejoin="round"/>
							<path d="M2 17l10 5 10-5M2 12l10 5 10-5" stroke-linecap="round" stroke-linejoin="round"/>
						</svg>
					</div>
					<div class="option-content">
						<span class="option-name">Русский</span>
						<span class="option-description">Russian language</span>
					</div>
					{#if language === 'ru'}
						<div class="option-check">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
								<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
							</svg>
						</div>
					{/if}
				</button>
			</div>
		</div>
	</section>
	
	<!-- Save Button -->
	<section class="settings-section">
		<div class="save-section">
			<Button variant="filled" size="large" onclick={saveSettings} disabled={isSaving}>
				{isSaving ? 'Saving...' : 'Save Settings'}
			</Button>
		</div>
	</section>
</div>

<style>
	.settings-page {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		max-width: 1200px;
	}

	.page-title {
		font: var(--typography-headline-medium);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.message-toast {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-4);
		border-radius: var(--shape-extra-large);
		font: var(--typography-body-medium);
		animation: slide-in var(--duration-normal) var(--ease-emphasized-decelerate);
	}

	.message-toast svg {
		width: 20px;
		height: 20px;
		flex-shrink: 0;
	}

	.message-toast.success {
		background: var(--color-profit-container);
		color: var(--color-profit);
	}

	.message-toast.error {
		background: var(--md-sys-color-error-container);
		color: var(--md-sys-color-error);
	}

	@keyframes slide-in {
		from {
			opacity: 0;
			transform: translateY(-10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.settings-section {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.settings-card {
		border-radius: var(--shape-large);
		overflow: hidden;
	}

	.card-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-4);
		padding: var(--space-5);
		border-bottom: 1px solid var(--glass-border);
	}

	.card-title {
		font: var(--typography-title-large);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.card-description {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
		margin: var(--space-1) 0 0 0;
	}

	.section-actions {
		display: flex;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	.text-btn {
		background: var(--md-sys-color-surface-container-low);
		border: 1px solid var(--md-sys-color-outline);
		color: var(--md-sys-color-primary);
		font: var(--typography-label-medium);
		font-weight: 500;
		cursor: pointer;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--shape-medium);
		transition: all var(--duration-fast) var(--ease-standard);
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	.text-btn:hover {
		background-color: var(--md-sys-color-primary-container);
		border-color: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary-container);
		transform: translateY(-1px);
		box-shadow: var(--elevation-1);
	}

	.text-btn:active {
		transform: translateY(0);
		box-shadow: none;
	}

	/* Execution Mode */
	.execution-options {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-4);
	}

	.execution-option {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		padding: var(--space-4);
		border-radius: var(--shape-large);
		border: 1px solid transparent;
		background: transparent;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
		text-align: left;
		width: 100%;
	}

	.execution-option:hover {
		background: var(--md-sys-color-surface-container-low);
	}

	.execution-option.active {
		border-color: var(--md-sys-color-primary);
		background: var(--md-sys-color-primary-container);
	}

	.option-icon {
		width: 48px;
		height: 48px;
		border-radius: var(--shape-medium);
		background: var(--md-sys-color-surface-container-high);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.execution-option.active .option-icon {
		background: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary);
	}

	.option-icon svg {
		width: 24px;
		height: 24px;
	}

	.option-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.option-name {
		font: var(--typography-title-medium);
		color: var(--md-sys-color-on-surface);
	}

	.option-description {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
	}

	.option-check {
		width: 24px;
		height: 24px;
		border-radius: var(--shape-full);
		background: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.option-check svg {
		width: 16px;
		height: 16px;
	}

	.auto-settings {
		padding: 0 var(--space-4) var(--space-4);
	}

	.setting-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		padding: var(--space-4);
		background: var(--md-sys-color-surface-container-low);
		border-radius: var(--shape-medium);
	}

	.setting-info {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.setting-label {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
	}

	.setting-description {
		font: var(--typography-body-small);
		color: var(--md-sys-color-on-surface-variant);
	}

	/* Theme Options */
	.theme-options {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-4);
	}

	.theme-option {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		padding: var(--space-4);
		border-radius: var(--shape-large);
		border: 1px solid transparent;
		background: transparent;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
		text-align: left;
		width: 100%;
	}

	.theme-option:hover {
		background: var(--md-sys-color-surface-container-low);
	}

	.theme-option.active {
		border-color: var(--md-sys-color-primary);
		background: var(--md-sys-color-primary-container);
	}

	.theme-option.active .option-icon {
		background: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary);
	}

	.slider {
		width: 200px;
		height: 4px;
		border-radius: var(--shape-full);
		background: var(--md-sys-color-surface-container-highest);
		appearance: none;
		cursor: pointer;
	}

	.slider::-webkit-slider-thumb {
		appearance: none;
		width: 20px;
		height: 20px;
		border-radius: var(--shape-full);
		background: var(--md-sys-color-primary);
		cursor: pointer;
	}
	
	/* Language Options */
	.language-options {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-4);
	}
	
	.language-option {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		padding: var(--space-4);
		border-radius: var(--shape-large);
		border: 1px solid transparent;
		background: transparent;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
		text-align: left;
		width: 100%;
	}
	
	.language-option:hover {
		background: var(--md-sys-color-surface-container-low);
	}
	
	.language-option.active {
		border-color: var(--md-sys-color-primary);
		background: var(--md-sys-color-primary-container);
	}
	
	.language-option.active .option-icon {
		background: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary);
	}
	
	/* Exchange Grid */
	.exchange-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
		gap: var(--space-3);
		padding: var(--space-4);
	}

	.exchange-item {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-4);
		border-radius: var(--shape-medium);
		border: 1px solid var(--glass-border);
		background: transparent;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
		text-align: left;
	}

	.exchange-item:hover {
		background: var(--md-sys-color-surface-container-low);
	}

	.exchange-item.enabled {
		border-color: var(--md-sys-color-primary);
		background: var(--md-sys-color-primary-container);
	}

	.exchange-logo {
		width: 40px;
		height: 40px;
		border-radius: var(--shape-small);
		background: var(--md-sys-color-surface-container-high);
		display: flex;
		align-items: center;
		justify-content: center;
		font: var(--typography-title-medium);
		font-weight: 600;
		color: var(--md-sys-color-on-surface-variant);
		flex-shrink: 0;
	}

	.exchange-logo.enabled {
		background: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary);
	}

	.exchange-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}

	.exchange-name {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.exchange-status {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
	}

	.exchange-check {
		width: 20px;
		height: 20px;
		border-radius: var(--shape-full);
		border: 2px solid var(--md-sys-color-outline);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.exchange-item.enabled .exchange-check {
		background: var(--md-sys-color-primary);
		border-color: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary);
	}

	.exchange-check svg {
		width: 12px;
		height: 12px;
	}

	/* Strategy Grid */
	.strategy-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: var(--space-3);
		padding: var(--space-4);
	}

	.strategy-item-wrapper {
		position: relative;
	}

	.strategy-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-4);
		border-radius: var(--shape-medium);
		border: 1px solid var(--glass-border);
		background: transparent;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
		text-align: left;
		width: 100%;
	}

	.strategy-item:hover {
		background: var(--md-sys-color-surface-container-low);
	}

	.strategy-item.enabled {
		border-color: var(--md-sys-color-primary);
		background: var(--md-sys-color-primary-container);
	}

	.strategy-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.strategy-name {
		font: var(--typography-title-small);
		color: var(--md-sys-color-on-surface);
		font-weight: 600;
	}

	.strategy-item.enabled .strategy-name {
		color: var(--md-sys-color-on-primary-container);
	}

	.info-trigger {
		width: 24px;
		height: 24px;
		border-radius: var(--shape-small);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
		flex-shrink: 0;
	}

	.info-trigger:hover {
		background: var(--md-sys-color-surface-container-high);
	}

	.info-trigger svg {
		width: 16px;
		height: 16px;
		color: var(--md-sys-color-on-surface-variant);
	}

	.strategy-description {
		font: var(--typography-body-small);
		color: var(--md-sys-color-on-surface-variant);
		margin: 0;
		line-height: 1.4;
	}

	.strategy-item.enabled .strategy-description {
		color: var(--md-sys-color-on-primary-container);
		opacity: 0.8;
	}

	.strategy-check {
		position: absolute;
		top: var(--space-4);
		right: var(--space-4);
		width: 20px;
		height: 20px;
		border-radius: var(--shape-full);
		border: 2px solid var(--md-sys-color-outline);
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0;
		transform: scale(0.8);
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.strategy-check.active {
		opacity: 1;
		transform: scale(1);
		background: var(--md-sys-color-primary);
		border-color: var(--md-sys-color-primary);
		color: var(--md-sys-color-on-primary);
	}

	.strategy-check svg {
		width: 12px;
		height: 12px;
	}

	.strategy-tooltip {
		position: absolute;
		top: 100%;
		left: 0;
		right: 0;
		z-index: 100;
		margin-top: var(--space-2);
		padding: var(--space-4);
		border-radius: var(--shape-medium);
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
		animation: tooltip-in var(--duration-fast) var(--ease-emphasized-decelerate);
	}

	@keyframes tooltip-in {
		from {
			opacity: 0;
			transform: translateY(-8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.tooltip-title {
		font: var(--typography-title-small);
		color: var(--md-sys-color-on-surface);
		margin: 0 0 var(--space-2) 0;
	}

	.tooltip-content {
		font: var(--typography-body-small);
		color: var(--md-sys-color-on-surface-variant);
		margin: 0;
		line-height: 1.5;
	}

	/* Risk Management */
	.risk-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
		gap: var(--space-4);
		padding: var(--space-5);
	}

	.risk-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.risk-label {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
	}

	.risk-control {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.glass-input {
		flex: 1;
		padding: var(--space-3) var(--space-4);
		border-radius: var(--shape-medium);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		color: var(--md-sys-color-on-surface);
		font: var(--typography-body-medium);
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.glass-input:focus {
		outline: none;
		border-color: var(--md-sys-color-primary);
		box-shadow: 0 0 0 3px var(--md-sys-color-primary-container);
	}

	.number-input {
		max-width: 140px;
	}

	.risk-unit {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
	}

	.save-section {
		display: flex;
		justify-content: flex-end;
	}
</style>
