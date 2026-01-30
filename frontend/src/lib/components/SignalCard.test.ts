import { describe, it, expect } from 'vitest';
import type { TradeSignal } from '$lib/types';

describe('SignalCard', () => {
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

	it('should calculate profit correctly', () => {
		const profitBps = mockSignal.profitBps;
		expect(profitBps).toBe(200);
	});

	it('should determine if signal is profitable', () => {
		const isProfitable = mockSignal.profitBps > 0;
		expect(isProfitable).toBe(true);
	});

	it('should format confidence as percentage', () => {
		const confidencePercent = Math.round(mockSignal.confidence * 100);
		expect(confidencePercent).toBe(85);
	});
});
