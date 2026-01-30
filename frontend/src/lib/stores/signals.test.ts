import { describe, it, expect, beforeEach } from 'vitest';
import { signalsStore } from './signals.svelte';
import type { TradeSignal } from '$lib/types';

describe('SignalsStore', () => {
	const mockSignal: TradeSignal = {
		id: '123e4567-e89b-12d3-a456-426614174000',
		strategy: 'cex-arbitrage',
		buyExchange: 'okx',
		sellExchange: 'bybit',
		symbol: 'BTC/USDT',
		buyPrice: 50000,
		sellPrice: 50100,
		volume: 1.5,
		profitBps: 200,
		confidence: 0.85,
		timestamp: Date.now()
	};

	beforeEach(() => {
		signalsStore.clearSignals();
	});

	it('should add signal to store', () => {
		signalsStore.addSignal(mockSignal);
		expect(signalsStore.signals.length).toBe(1);
		expect(signalsStore.signals[0].id).toBe(mockSignal.id);
	});

	it('should filter signals by strategy', () => {
		signalsStore.addSignal(mockSignal);
		signalsStore.addSignal({ ...mockSignal, id: '2', strategy: 'funding-arbitrage' });
		
		signalsStore.setFilter('cex-arbitrage');
		expect(signalsStore.filteredSignals.length).toBe(1);
		expect(signalsStore.filteredSignals[0].strategy).toBe('cex-arbitrage');
	});

	it('should sort signals by profit', () => {
		signalsStore.addSignal({ ...mockSignal, profitBps: 100 });
		signalsStore.addSignal({ ...mockSignal, id: '2', profitBps: 300 });
		signalsStore.addSignal({ ...mockSignal, id: '3', profitBps: 200 });
		
		const sorted = signalsStore.sortedSignals;
		expect(sorted[0].profitBps).toBe(300);
		expect(sorted[1].profitBps).toBe(200);
		expect(sorted[2].profitBps).toBe(100);
	});

	it('should limit signals to maxSignals', () => {
		for (let i = 0; i < 150; i++) {
			signalsStore.addSignal({ ...mockSignal, id: `signal-${i}` });
		}
		expect(signalsStore.signals.length).toBe(100);
	});

	it('should update existing signal', () => {
		signalsStore.addSignal(mockSignal);
		signalsStore.updateSignal(mockSignal.id, { profitBps: 250 });
		
		expect(signalsStore.signals[0].profitBps).toBe(250);
	});

	it('should clear all signals', () => {
		signalsStore.addSignal(mockSignal);
		signalsStore.clearSignals();
		expect(signalsStore.signals.length).toBe(0);
	});
});
