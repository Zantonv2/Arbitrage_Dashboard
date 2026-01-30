import type { TradeSignal, StrategyType } from '$lib/types';

class SignalsStore {
	signals = $state<TradeSignal[]>([]);
	selectedStrategy = $state<StrategyType | null>(null);
	maxSignals = 100;

	filteredSignals = $derived(
		this.selectedStrategy
			? this.signals.filter((s) => s.strategy === this.selectedStrategy)
			: this.signals
	);

	sortedSignals = $derived(
		[...this.filteredSignals].sort((a, b) => b.profitBps - a.profitBps)
	);

	addSignal(signal: TradeSignal): void {
		this.signals = [signal, ...this.signals.slice(0, this.maxSignals - 1)];
	}

	updateSignal(signalId: string, updates: Partial<TradeSignal>): void {
		const index = this.signals.findIndex((s) => s.id === signalId);
		if (index !== -1) {
			this.signals[index] = { ...this.signals[index], ...updates };
		}
	}

	clearSignals(): void {
		this.signals = [];
	}

	setFilter(strategy: StrategyType | null): void {
		this.selectedStrategy = strategy;
	}
}

export const signalsStore = new SignalsStore();
