<script lang="ts">
	import { systemStore } from '$lib/stores/system.svelte';
	import Button from './ui/Button.svelte';

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

<div class="execution-mode-toggle bg-white dark:bg-gray-800 rounded-lg p-6 border border-gray-200 dark:border-gray-700">
	<h2 class="text-xl font-semibold mb-4">Execution Mode</h2>

	<div class="flex gap-4 mb-6" role="radiogroup" aria-label="Execution mode">
		<button
			class="mode-button flex-1 px-4 py-3 rounded-lg border-2 transition-all {systemStore.mode === 'manual' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20' : 'border-gray-300 dark:border-gray-600'}"
			onclick={() => handleModeChange('manual')}
			role="radio"
			aria-checked={systemStore.mode === 'manual'}
		>
			<div class="font-semibold">Manual</div>
			<div class="text-sm text-gray-600 dark:text-gray-400">Review each signal</div>
		</button>

		<button
			class="mode-button flex-1 px-4 py-3 rounded-lg border-2 transition-all {systemStore.mode === 'auto' ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20' : 'border-gray-300 dark:border-gray-600'}"
			onclick={() => handleModeChange('auto')}
			role="radio"
			aria-checked={systemStore.mode === 'auto'}
		>
			<div class="font-semibold">Auto</div>
			<div class="text-sm text-gray-600 dark:text-gray-400">Execute automatically</div>
		</button>
	</div>

	{#if systemStore.mode === 'auto'}
		<div class="confidence-threshold mb-6">
			<label for="confidence-slider" class="block text-sm font-medium mb-2">
				Confidence Threshold: {Math.round(systemStore.confidenceThreshold * 100)}%
			</label>
			<input
				id="confidence-slider"
				type="range"
				min="0.5"
				max="1"
				step="0.05"
				bind:value={systemStore.confidenceThreshold}
				class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer"
			/>
			<div class="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
				<span>50%</span>
				<span>100%</span>
			</div>
		</div>
	{/if}

	<div class="emergency-stop">
		<Button variant="danger" size="lg" onclick={handleEmergencyStop}>
			{#snippet children()}
				🛑 Emergency Stop
			{/snippet}
		</Button>
	</div>

	<div class="mt-4 text-sm text-gray-600 dark:text-gray-400" role="status" aria-live="polite">
		Current mode: <strong>{systemStore.mode === 'auto' ? 'Automatic' : 'Manual'}</strong>
	</div>
</div>

{#if showConfirmDialog}
	<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" role="dialog" aria-modal="true" aria-labelledby="confirm-dialog-title">
		<div class="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md mx-4">
			<h3 id="confirm-dialog-title" class="text-xl font-semibold mb-4">Enable Auto Mode?</h3>
			<p class="text-gray-600 dark:text-gray-400 mb-6">
				Auto mode will automatically execute trades that meet your confidence threshold. This action involves real financial risk.
			</p>
			<div class="flex gap-3">
				<Button variant="secondary" onclick={cancelAutoMode}>
					{#snippet children()}
						Cancel
					{/snippet}
				</Button>
				<Button variant="primary" onclick={confirmAutoMode}>
					{#snippet children()}
						Confirm
					{/snippet}
				</Button>
			</div>
		</div>
	</div>
{/if}
