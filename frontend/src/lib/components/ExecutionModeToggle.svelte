<script lang="ts">
	import { systemStore } from '$lib/stores/system.svelte';
	import Button from './ui/Button.svelte';
	import Badge from './ui/Badge.svelte';

	let showConfirmDialog = $state(false);
	let pendingMode: 'auto' | 'manual' | null = $state(null);

	function handleModeChange(newMode: 'auto' | 'manual'): void {
		if (newMode === 'auto') {
			pendingMode = newMode;
			showConfirmDialog = true;
		} else {
			systemStore.setMode(newMode);
		}
	}

	function confirmAutoMode(): void {
		if (pendingMode === 'auto') {
			systemStore.setMode('auto');
		}
		showConfirmDialog = false;
		pendingMode = null;
	}

	function cancelAutoMode(): void {
		showConfirmDialog = false;
		pendingMode = null;
	}

	function handleEmergencyStop(): void {
		systemStore.setMode('manual');
		systemStore.disableAllStrategies();
	}
</script>

<div class="execution-mode-toggle glass">
	<div class="toggle-header">
		<h2 class="toggle-title">Execution Mode</h2>
		<Badge 
			variant={systemStore.mode === 'auto' ? 'filled' : 'tonal'} 
			color={systemStore.mode === 'auto' ? 'profit' : 'default'}
			size="medium"
		>
			{systemStore.mode === 'auto' ? 'Automatic' : 'Manual'}
		</Badge>
	</div>

	<div class="mode-buttons" role="radiogroup" aria-label="Execution mode">
		<button
			class="mode-btn glass-interactive"
			class:active={systemStore.mode === 'manual'}
			onclick={() => handleModeChange('manual')}
			role="radio"
			aria-checked={systemStore.mode === 'manual'}
		>
			<div class="mode-content">
				<svg class="mode-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 11c0 3.517-1.009 6.799-2.753 9.571m-3.44-2.04l.054-.09A13.916 13.916 0 008 11a4 4 0 118 0c0 1.017-.07 2.019-.203 3m-2.118 6.844A21.88 21.88 0 0015.171 17m3.839 1.132c.645-2.266.99-4.659.99-7.132A8 8 0 008 4.07M3 15.364c.64-1.319 1-2.8 1-4.364 0-1.457.39-2.823 1.07-4" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				<div class="mode-info">
					<span class="mode-name">Manual</span>
					<span class="mode-description">Review each signal</span>
				</div>
			</div>
			{#if systemStore.mode === 'manual'}
				<div class="active-indicator">
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
						<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</div>
			{/if}
		</button>

		<button
			class="mode-btn glass-interactive"
			class:active={systemStore.mode === 'auto'}
			onclick={() => handleModeChange('auto')}
			role="radio"
			aria-checked={systemStore.mode === 'auto'}
		>
			<div class="mode-content">
				<svg class="mode-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				<div class="mode-info">
					<span class="mode-name">Auto</span>
					<span class="mode-description">Execute automatically</span>
				</div>
			</div>
			{#if systemStore.mode === 'auto'}
				<div class="active-indicator">
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
						<path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				</div>
			{/if}
		</button>
	</div>

	{#if systemStore.mode === 'auto'}
		<div class="confidence-section glass-interactive">
			<label for="confidence-slider" class="confidence-label">
				Confidence Threshold: <strong>{Math.round(systemStore.confidenceThreshold * 100)}%</strong>
			</label>
			<input
				id="confidence-slider"
				type="range"
				min="0.5"
				max="1"
				step="0.05"
				bind:value={systemStore.confidenceThreshold}
				class="confidence-slider"
			/>
			<div class="slider-labels">
				<span>50%</span>
				<span>100%</span>
			</div>
		</div>
	{/if}

	<div class="emergency-section">
		<Button variant="filled" size="large" onclick={handleEmergencyStop}>
			<svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
			</svg>
			Emergency Stop
		</Button>
	</div>

	<div class="status-bar" role="status" aria-live="polite">
		<span class="status-text">
			Current mode: <strong>{systemStore.mode === 'auto' ? 'Automatic' : 'Manual'}</strong>
		</span>
	</div>
</div>

{#if showConfirmDialog}
	<div class="dialog-overlay" role="dialog" aria-modal="true" aria-labelledby="confirm-dialog-title">
		<div class="dialog-card glass">
			<h3 id="confirm-dialog-title" class="dialog-title">Enable Auto Mode?</h3>
			<p class="dialog-message">
				Auto mode will automatically execute trades that meet your confidence threshold. 
				This action involves real financial risk.
			</p>
			<div class="dialog-actions">
				<Button variant="tonal" onclick={cancelAutoMode}>
					Cancel
				</Button>
				<Button variant="filled" onclick={confirmAutoMode}>
					Confirm
				</Button>
			</div>
		</div>
	</div>
{/if}

<style>
	.execution-mode-toggle {
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

	.toggle-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.toggle-title {
		font: var(--typography-title-medium);
		color: var(--md-sys-color-on-surface);
		margin: 0;
	}

	.mode-buttons {
		display: flex;
		gap: var(--space-3);
	}

	.mode-btn {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-4);
		border-radius: var(--shape-large);
		border: 1px solid var(--glass-border);
		background: transparent;
		cursor: pointer;
		transition: all var(--duration-fast) var(--ease-standard);
	}

	.mode-btn:hover {
		background: var(--glass-background-subtle);
		border-color: var(--glass-border-strong);
	}

	.mode-btn.active {
		background: var(--md-sys-color-primary-container);
		border-color: var(--md-sys-color-primary);
	}

	.mode-content {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}

	.mode-icon {
		width: 24px;
		height: 24px;
		color: var(--md-sys-color-on-surface-variant);
	}

	.mode-btn.active .mode-icon {
		color: var(--md-sys-color-on-primary-container);
	}

	.mode-info {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
	}

	.mode-name {
		font: var(--typography-body-medium);
		font-weight: 600;
		color: var(--md-sys-color-on-surface);
	}

	.mode-btn.active .mode-name {
		color: var(--md-sys-color-on-primary-container);
	}

	.mode-description {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
	}

	.mode-btn.active .mode-description {
		color: var(--md-sys-color-on-primary-container);
		opacity: 0.7;
	}

	.active-indicator {
		width: 24px;
		height: 24px;
		border-radius: var(--shape-full);
		background: var(--md-sys-color-primary);
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--md-sys-color-on-primary);
	}

	.active-indicator svg {
		width: 14px;
		height: 14px;
	}

	.confidence-section {
		padding: var(--space-4);
		border-radius: var(--shape-large);
		border: 1px solid var(--glass-border);
	}

	.confidence-label {
		display: block;
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface);
		margin-bottom: var(--space-3);
	}

	.confidence-slider {
		width: 100%;
		height: 8px;
		border-radius: var(--shape-full);
		background: var(--md-sys-color-surface-container-high);
		appearance: none;
		cursor: pointer;
	}

	.confidence-slider::-webkit-slider-thumb {
		appearance: none;
		width: 24px;
		height: 24px;
		border-radius: var(--shape-full);
		background: var(--md-sys-color-primary);
		cursor: pointer;
		box-shadow: 0 2px 8px rgba(103, 80, 164, 0.3);
		transition: transform var(--duration-fast) var(--ease-standard);
	}

	.confidence-slider::-webkit-slider-thumb:hover {
		transform: scale(1.1);
	}

	.slider-labels {
		display: flex;
		justify-content: space-between;
		margin-top: var(--space-2);
	}

	.slider-labels span {
		font: var(--typography-label-small);
		color: var(--md-sys-color-on-surface-variant);
	}

	.emergency-section {
		display: flex;
		justify-content: center;
	}

	.btn-icon {
		width: 20px;
		height: 20px;
	}

	.status-bar {
		padding-top: var(--space-3);
		border-top: 1px solid var(--glass-border);
	}

	.status-text {
		font: var(--typography-body-small);
		color: var(--md-sys-color-on-surface-variant);
	}

	.status-text strong {
		color: var(--md-sys-color-on-surface);
	}

	.dialog-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 50;
		backdrop-filter: blur(4px);
	}

	.dialog-card {
		width: 100%;
		max-width: 420px;
		margin: var(--space-4);
		padding: var(--space-6);
		border-radius: var(--shape-extra-large);
		border: 1px solid var(--glass-border);
		background: var(--glass-background);
		backdrop-filter: var(--glass-backdrop);
		-webkit-backdrop-filter: var(--glass-backdrop);
	}

	.dialog-title {
		font: var(--typography-title-large);
		color: var(--md-sys-color-on-surface);
		margin: 0 0 var(--space-3) 0;
	}

	.dialog-message {
		font: var(--typography-body-medium);
		color: var(--md-sys-color-on-surface-variant);
		margin: 0 0 var(--space-5) 0;
	}

	.dialog-actions {
		display: flex;
		gap: var(--space-3);
		justify-content: flex-end;
	}
</style>
