export type ExchangeId = 'okx' | 'bybit' | 'mexc' | 'gateio' | 'kraken' | 'bitstamp';

export type StrategyType =
	| 'cex-arbitrage'
	| 'funding-arbitrage'
	| 'spot-perp-arbitrage'
	| 'stablecoin-arbitrage'
	| 'cross-exchange-arbitrage'
	| 'spread-capture'
	| 'latency-arbitrage'
	| 'convergence-arbitrage'
	| 'new-listing-arbitrage'
	| 'hedged-funding';

export interface TradeSignal {
	id: string;
	strategy: StrategyType;
	buyExchange: ExchangeId;
	sellExchange: ExchangeId;
	symbol: string;
	buyPrice: number;
	sellPrice: number;
	volume: number;
	profitBps: number;
	confidence: number;
	timestamp: number;
}

export interface ExchangeStatus {
	exchange: ExchangeId;
	status: 'online' | 'offline' | 'degraded';
	latency: number;
	lastUpdate: number;
	rateLimitUsage: number;
}

export interface OrderBookUpdate {
	exchange: ExchangeId;
	symbol: string;
	bids: [number, number][];
	asks: [number, number][];
	timestamp: number;
}

export interface ExecutionUpdate {
	signalId: string;
	status: 'pending' | 'executing' | 'completed' | 'failed';
	message?: string;
	timestamp: number;
}

export interface SystemStatus {
	mode: 'auto' | 'manual';
	confidenceThreshold: number;
	enabledStrategies: StrategyType[];
	timestamp: number;
}

export type WebSocketMessage =
	| { type: 'signal_update'; data: TradeSignal }
	| { type: 'orderbook_update'; data: OrderBookUpdate }
	| { type: 'execution_update'; data: ExecutionUpdate }
	| { type: 'exchange_status'; data: ExchangeStatus }
	| { type: 'system_status'; data: SystemStatus };

export interface ApiResponse<T> {
	data: T;
	status: 'success' | 'error';
	message?: string;
	timestamp: number;
}
