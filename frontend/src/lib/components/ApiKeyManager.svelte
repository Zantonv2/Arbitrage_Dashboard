<script lang="ts">
	import Button from '$lib/components/ui/Button.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import { apiClient } from '$lib/api/client';
	import { t, tp } from '$lib/i18n.svelte';

	interface CredentialStatus {
		exchange: string;
		has_credentials: boolean;
		testnet: boolean;
		validated: boolean;
	}

	let credentials = $state<CredentialStatus[]>([]);
	let loading = $state(true);
	let saving = $state(false);
	let validating = $state(false);
	let error = $state<string | null>(null);
	let success = $state<string | null>(null);

	// Form state
	let selectedExchange = $state<string | null>(null);
	let apiKey = $state('');
	let apiSecret = $state('');
	let passphrase = $state('');
	let testnet = $state(true);

	const exchanges = ['okx', 'bybit', 'mexc', 'gateio', 'kraken', 'bitstamp'];

	async function loadCredentials() {
		loading = true;
		error = null;
		try {
			const result = await apiClient.getCredentials();
			credentials = result.credentials;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load credentials';
		} finally {
			loading = false;
		}
	}

	function openModal(exchange: string) {
		selectedExchange = exchange;
		apiKey = '';
		apiSecret = '';
		passphrase = '';
		const existing = credentials.find(c => c.exchange === exchange);
		testnet = existing?.testnet ?? true;
		error = null;
		success = null;
	}

	function closeModal() {
		selectedExchange = null;
		apiKey = '';
		apiSecret = '';
		passphrase = '';
		error = null;
		success = null;
	}

	async function saveCredentials() {
		if (!selectedExchange) return;
		if (!apiKey.trim() || !apiSecret.trim()) {
			error = 'API Key and Secret are required';
			return;
		}

		saving = true;
		error = null;
		success = null;

		try {
			const result = await apiClient.saveCredentials({
				exchange: selectedExchange,
				api_key: apiKey,
				api_secret: apiSecret,
				passphrase: passphrase || undefined
			});
			success = result.message;
			await loadCredentials();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save credentials';
		} finally {
			saving = false;
		}
	}

	async function validateCredentials() {
		if (!selectedExchange) return;
		if (!apiKey.trim() || !apiSecret.trim()) {
			error = 'API Key and Secret are required';
			return;
		}

		validating = true;
		error = null;
		success = null;

		try {
			const result = await apiClient.validateCredentials({
				exchange: selectedExchange,
				api_key: apiKey,
				api_secret: apiSecret,
				passphrase: passphrase || undefined,
				testnet
			});
			success = result.message;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to validate credentials';
		} finally {
			validating = false;
		}
	}

	async function deleteCredentials(exchange: string) {
		if (!confirm(tp('credentials.deleteConfirm', { exchange }))) return;

		try {
			await apiClient.deleteCredentials(exchange);
			await loadCredentials();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete credentials';
		}
	}

	async function toggleMode(exchange: string, newMode: boolean) {
		try {
			await apiClient.toggleExchangeMode(exchange, newMode);
			await loadCredentials();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to toggle mode';
		}
	}

	function getExchangeLabel(exchange: string): string {
		return exchange.charAt(0).toUpperCase() + exchange.slice(1);
	}

	// Load credentials on mount
	$effect(() => {
		loadCredentials();
	});
</script>

<div class="api-key-manager">
	<div class="header">
		<h2>{t('credentials.title')}</h2>
		<Button variant="tonal" onclick={loadCredentials} disabled={loading}>
			{t('credentials.refresh')}
		</Button>
	</div>

	{#if error}
		<div class="error-message">
			{error}
		</div>
	{/if}

	{#if success}
		<div class="success-message">
			{success}
		</div>
	{/if}

	{#if loading}
		<div class="loading">
			{t('credentials.loading')}
		</div>
	{:else}
		<div class="credentials-grid">
			{#each exchanges as exchange}
				{@const cred = credentials.find(c => c.exchange === exchange)}
				<div class="exchange-card">
					<div class="exchange-header">
						<h3>{getExchangeLabel(exchange)}</h3>
						<Badge
							variant="tonal"
							color={cred?.has_credentials ? 'profit' : 'default'}
						>
							{cred?.has_credentials ? t('credentials.configured') : t('credentials.notConfigured')}
						</Badge>
					</div>

					<div class="exchange-status">
						<div class="status-row">
							<span class="label">Mode:</span>
							<button
								class="mode-toggle"
								class:testnet={cred?.testnet}
								class:production={!cred?.testnet}
								onclick={() => toggleMode(exchange, !cred?.testnet)}
							>
								{cred?.testnet ? t('credentials.testnet') : t('credentials.production')}
							</button>
						</div>
						<div class="status-row">
							<span class="label">Validated:</span>
							<Badge variant="tonal" color={cred?.validated ? 'profit' : 'default'}>
								{cred?.validated ? t('credentials.validated') : t('credentials.notValidated')}
							</Badge>
						</div>
					</div>

					<div class="exchange-actions">
						<Button
							variant="filled"
							size="small"
							onclick={() => openModal(exchange)}
						>
							{cred?.has_credentials ? 'Edit' : 'Add'} Keys
						</Button>
						{#if cred?.has_credentials}
							<Button
								variant="outlined"
								size="small"
								onclick={() => deleteCredentials(exchange)}
							>
								Remove
							</Button>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Modal -->
{#if selectedExchange}
	<div class="modal-overlay" onclick={closeModal} role="dialog" aria-modal="true" tabindex="-1">
		<div class="modal" onclick={(e) => e.stopPropagation()}>
			<div class="modal-header">
				<h3>{tp('credentials.modalTitle', { exchange: getExchangeLabel(selectedExchange) })}</h3>
				<button class="close-btn" onclick={closeModal} aria-label="Close">
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</button>
			</div>

			<div class="modal-body">
				<div class="form-group">
					<label for="api-key">{t('credentials.apiKey')}</label>
					<input
						id="api-key"
						type="text"
						bind:value={apiKey}
						placeholder="Enter API Key"
						autocomplete="off"
					/>
				</div>

				<div class="form-group">
					<label for="api-secret">{t('credentials.apiSecret')}</label>
					<input
						id="api-secret"
						type="password"
						bind:value={apiSecret}
						placeholder="Enter API Secret"
						autocomplete="off"
					/>
				</div>

				{#if selectedExchange === 'okx' || selectedExchange === 'gateio'}
					<div class="form-group">
						<label for="passphrase">{t('credentials.passphrase')}</label>
						<input
							id="passphrase"
							type="password"
							bind:value={passphrase}
							placeholder="Enter Passphrase (optional)"
							autocomplete="off"
						/>
					</div>
				{/if}

				<div class="form-group toggle-group">
					<label class="toggle-label">
						<input type="checkbox" bind:checked={testnet} />
						<span class="toggle-text">{t('credentials.testnet')}</span>
					</label>
				</div>

				{#if error}
					<div class="error-message">{error}</div>
				{/if}

				{#if success}
					<div class="success-message">{success}</div>
				{/if}
			</div>

			<div class="modal-footer">
				<Button variant="outlined" onclick={validateCredentials} disabled={validating}>
					{validating ? 'Validating...' : 'Validate'}
				</Button>
				<Button variant="filled" onclick={saveCredentials} disabled={saving}>
					{saving ? 'Saving...' : 'Save'}
				</Button>
			</div>
		</div>
	</div>
{/if}

<style>
	.api-key-manager {
		padding: var(--space-4);
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: var(--space-4);
	}

	.header h2 {
		font: var(--typography-title-large);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.error-message {
		background-color: var(--color-error-container);
		color: var(--color-error);
		padding: var(--space-3);
		border-radius: var(--shape-medium);
		margin-bottom: var(--space-4);
		font: var(--typography-body-medium);
	}

	.success-message {
		background-color: var(--color-profit-container);
		color: var(--color-profit);
		padding: var(--space-3);
		border-radius: var(--shape-medium);
		margin-bottom: var(--space-4);
		font: var(--typography-body-medium);
	}

	.loading {
		text-align: center;
		padding: var(--space-8);
		color: var(--md-sys-color-on-surface-variant);
	}

	.credentials-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: var(--space-4);
	}

	.exchange-card {
		background: var(--glass-background);
		border: 1px solid var(--glass-border);
		border-radius: var(--shape-large);
		padding: var(--space-4);
		backdrop-filter: var(--glass-backdrop);
	}

	.exchange-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: var(--space-3);
	}

	.exchange-header h3 {
		font: var(--typography-title-medium);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.exchange-status {
		margin-bottom: var(--space-3);
	}

	.status-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: var(--space-2) 0;
	}

	.label {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
	}

	.mode-toggle {
		font: var(--typography-label-medium);
		padding: var(--space-1) var(--space-2);
		border-radius: var(--shape-small);
		border: 1px solid;
		cursor: pointer;
		transition: all var(--duration-fast);
		background: none;
	}

	.mode-toggle.testnet {
		background-color: var(--md-sys-color-tertiary-container);
		color: var(--md-sys-color-on-tertiary-container);
		border-color: var(--md-sys-color-tertiary);
	}

	.mode-toggle.production {
		background-color: var(--md-sys-color-primary-container);
		color: var(--md-sys-color-on-primary-container);
		border-color: var(--md-sys-color-primary);
	}

	.exchange-actions {
		display: flex;
		gap: var(--space-2);
	}

	/* Modal Styles */
	.modal-overlay {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.modal {
		background: var(--md-sys-color-surface);
		border-radius: var(--shape-extra-large);
		width: 90%;
		max-width: 480px;
		max-height: 90vh;
		overflow: auto;
	}

	.modal-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: var(--space-4);
		border-bottom: 1px solid var(--glass-border);
	}

	.modal-header h3 {
		font: var(--typography-title-large);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.close-btn {
		background: none;
		border: none;
		padding: var(--space-2);
		cursor: pointer;
		color: var(--md-sys-color-on-surface-variant);
		border-radius: var(--shape-full);
		transition: background-color var(--duration-fast);
	}

	.close-btn:hover {
		background: var(--md-sys-color-surface-variant);
	}

	.close-btn svg {
		width: 24px;
		height: 24px;
	}

	.modal-body {
		padding: var(--space-4);
	}

	.form-group {
		margin-bottom: var(--space-4);
	}

	.form-group label {
		display: block;
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
		margin-bottom: var(--space-2);
	}

	.form-group input {
		width: 100%;
		padding: var(--space-3);
		border: 1px solid var(--glass-border);
		border-radius: var(--shape-medium);
		background: var(--md-sys-color-surface-container);
		color: var(--md-sys-color-on-surface);
		font: var(--typography-body-large);
		transition: border-color var(--duration-fast);
	}

	.form-group input:focus {
		outline: none;
		border-color: var(--md-sys-color-primary);
	}

	.toggle-group {
		display: flex;
		align-items: center;
	}

	.toggle-label {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		cursor: pointer;
	}

	.toggle-label input {
		width: auto;
	}

	.toggle-text {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
	}

	.modal-footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		padding: var(--space-4);
		border-top: 1px solid var(--glass-border);
	}
</style>
