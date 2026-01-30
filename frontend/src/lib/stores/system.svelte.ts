import type { SystemStatus, StrategyType } from '$lib/types';

class SystemStore {
	mode = $state<'auto' | 'manual'>('manual');
	confidenceThreshold = $state(0.7);
	enabledStrategies = $state<StrategyType[]>([]);

	updateStatus(status: SystemStatus): void {
		this.mode = status.mode;
		this.confidenceThreshold = status.confidenceThreshold;
		this.enabledStrategies = status.enabledStrategies;
	}

	setMode(mode: 'auto' | 'manual'): void {
		this.mode = mode;
	}

	setConfidenceThreshold(threshold: number): void {
		this.confidenceThreshold = Math.max(0, Math.min(1, threshold));
	}

	toggleStrategy(strategy: StrategyType): void {
		const index = this.enabledStrategies.indexOf(strategy);
		if (index === -1) {
			this.enabledStrategies = [...this.enabledStrategies, strategy];
		} else {
			this.enabledStrategies = this.enabledStrategies.filter((s) => s !== strategy);
		}
	}

	enableAllStrategies(strategies: StrategyType[]): void {
		this.enabledStrategies = [...strategies];
	}

	disableAllStrategies(): void {
		this.enabledStrategies = [];
	}
}

export const systemStore = new SystemStore();
