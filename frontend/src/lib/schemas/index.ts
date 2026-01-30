import { z } from 'zod';

export const ExchangeIdSchema = z.enum(['okx', 'bybit', 'mexc', 'gateio', 'kraken', 'bitstamp']);

export const StrategyTypeSchema = z.enum([
	'cex-arbitrage',
	'funding-arbitrage',
	'spot-perp-arbitrage',
	'stablecoin-arbitrage',
	'cross-exchange-arbitrage',
	'spread-capture',
	'latency-arbitrage',
	'convergence-arbitrage',
	'new-listing-arbitrage',
	'hedged-funding'
]);

export const TradeSignalSchema = z.object({
	id: z.string().uuid(),
	strategy: StrategyTypeSchema,
	buyExchange: ExchangeIdSchema,
	sellExchange: ExchangeIdSchema,
	symbol: z.string(),
	buyPrice: z.number(),
	sellPrice: z.number(),
	volume: z.number(),
	profitBps: z.number(),
	confidence: z.number().min(0).max(1),
	timestamp: z.number()
});

export const ExchangeStatusSchema = z.object({
	exchange: ExchangeIdSchema,
	status: z.enum(['online', 'offline', 'degraded']),
	latency: z.number(),
	lastUpdate: z.number(),
	rateLimitUsage: z.number().min(0).max(1)
});

export const OrderBookUpdateSchema = z.object({
	exchange: ExchangeIdSchema,
	symbol: z.string(),
	bids: z.array(z.tuple([z.number(), z.number()])),
	asks: z.array(z.tuple([z.number(), z.number()])),
	timestamp: z.number()
});

export const ExecutionUpdateSchema = z.object({
	signalId: z.string().uuid(),
	status: z.enum(['pending', 'executing', 'completed', 'failed']),
	message: z.string().optional(),
	timestamp: z.number()
});

export const SystemStatusSchema = z.object({
	mode: z.enum(['auto', 'manual']),
	confidenceThreshold: z.number().min(0).max(1),
	enabledStrategies: z.array(StrategyTypeSchema),
	timestamp: z.number()
});
