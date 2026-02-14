import { z } from 'zod';
import {
	TradeSignalSchema,
	ExchangeStatusSchema,
	SystemStatusSchema
} from '$lib/schemas';
import type { TradeSignal, ExchangeStatus, ApiResponse } from '$lib/types';

const API_BASE_URL = '/api';

class ApiClient {
	private async request<T>(
		endpoint: string,
		options?: RequestInit,
		schema?: z.ZodSchema<T>
	): Promise<T> {
		const response = await fetch(`${API_BASE_URL}${endpoint}`, {
			...options,
			headers: {
				'Content-Type': 'application/json',
				...options?.headers
			}
		});

		if (!response.ok) {
			throw new Error(`API Error: ${response.status} ${response.statusText}`);
		}

		const data = await response.json();

		if (schema) {
			return schema.parse(data);
		}

		return data as T;
	}

	// Signals
	async getSignals(params?: {
		limit?: number;
		offset?: number;
		symbol?: string;
		exchange?: string;
		min_profit?: number;
		min_confidence?: number;
	}): Promise<TradeSignal[]> {
		const queryParams = new URLSearchParams();
		if (params) {
			Object.entries(params).forEach(([key, value]) => {
				if (value !== undefined) {
					queryParams.append(key, value.toString());
				}
			});
		}

		const query = queryParams.toString();
		const endpoint = query ? `/signals?${query}` : '/signals';

		const response = await this.request<{ signals: unknown[] }>(endpoint);
		return z.array(TradeSignalSchema).parse(response.signals);
	}

	async getSignal(id: string): Promise<TradeSignal> {
		const response = await this.request<{ signal: unknown }>(`/signals/${id}`);
		return TradeSignalSchema.parse(response.signal);
	}

	// Exchange Status
	async getExchangeStatus(): Promise<ExchangeStatus[]> {
		const response = await this.request<{ exchanges: unknown[] }>('/status/exchanges');
		return z.array(ExchangeStatusSchema).parse(response.exchanges);
	}

	// Order Book
	async getOrderBook(exchange: string, symbol: string): Promise<{
		exchange: string;
		symbol: string;
		bids: [number, number][];
		asks: [number, number][];
	}> {
		return this.request(`/orderbooks/${exchange}/${symbol}`);
	}

	// Execution
	async prepareExecution(signalId: string, quantity?: number): Promise<{
		instruction_id: string;
		buy_order: unknown;
		sell_order: unknown;
		expected_profit: number;
		worst_case_profit: number;
		total_fees: number;
		is_valid: boolean;
		validation_errors: string[];
	}> {
		return this.request('/executions/prepare', {
			method: 'POST',
			body: JSON.stringify({
				signal_id: signalId,
				quantity
			})
		});
	}

	async confirmExecution(instructionId: string): Promise<{
		success: boolean;
		message: string;
		execution_id?: string;
		buy_order?: {
			order_id: string;
			exchange: string;
			symbol: string;
			side: string;
			quantity: number;
			price: number;
			status: string;
		};
		sell_order?: {
			order_id: string;
			exchange: string;
			symbol: string;
			side: string;
			quantity: number;
			price: number;
			status: string;
		};
		actual_profit?: number;
		execution_time_ms?: number;
		rollback_performed: boolean;
	}> {
		return this.request('/executions/confirm', {
			method: 'POST',
			body: JSON.stringify({
				instruction_id: instructionId
			})
		});
	}

	// Analytics
	async getAnalytics(period?: string): Promise<{
		total_signals_detected: number;
		total_signals_filtered: number;
		total_signals_emitted: number;
		active_symbols: number;
		cached_order_books: number;
		period: string;
	}> {
		const query = period ? `?period=${period}` : '';
		return this.request(`/analytics${query}`);
	}

	// Configuration
	async getConfig(): Promise<unknown> {
		return this.request('/config');
	}

	async updateConfig(config: unknown): Promise<{
		success: boolean;
		message: string;
	}> {
		return this.request('/config', {
			method: 'POST',
			body: JSON.stringify(config)
		});
	}

	// Audit Log
	async getAuditLog(limit?: number): Promise<{
		entries: Array<{
			id: string;
			timestamp: string;
			decision: string;
			status: string;
			message: string;
			opportunity_id?: string;
			symbol?: string;
			exchanges?: string[];
			profit?: string;
		}>;
		total: number;
		limit: number;
	}> {
		const query = limit ? `?limit=${limit}` : '';
		return this.request(`/audit${query}`);
	}

	// Bridge Status
	async getBridgeStatus(): Promise<{
		bridge: {
			connected_exchanges: number;
			total_exchanges: number;
			total_messages_received: number;
			total_errors: number;
			active_symbols: number;
		};
		engine: {
			cached_signals: number;
			order_books_cached: number;
			tickers_cached: number;
			funding_rates_cached: number;
			signals_detected: number;
			signals_emitted: number;
		};
		last_updated: string;
	}> {
		return this.request('/status/bridge');
	}

	// Credentials Management
	async getCredentials(): Promise<{
		credentials: Array<{
			exchange: string;
			has_credentials: boolean;
			testnet: boolean;
			validated: boolean;
		}>;
	}> {
		return this.request('/credentials');
	}

	async saveCredentials(credentials: {
		exchange: string;
		api_key: string;
		api_secret: string;
		passphrase?: string;
	}): Promise<{ success: boolean; message: string }> {
		return this.request('/credentials', {
			method: 'POST',
			body: JSON.stringify(credentials)
		});
	}

	async validateCredentials(credentials: {
		exchange: string;
		api_key: string;
		api_secret: string;
		passphrase?: string;
		testnet: boolean;
	}): Promise<{ success: boolean; message: string }> {
		return this.request('/credentials/validate', {
			method: 'POST',
			body: JSON.stringify(credentials)
		});
	}

	async deleteCredentials(exchange: string): Promise<{ success: boolean; message: string }> {
		return this.request(`/credentials/${exchange}`, {
			method: 'DELETE'
		});
	}

	async toggleExchangeMode(exchange: string, testnet: boolean): Promise<{ success: boolean; message: string }> {
		return this.request('/exchange/mode', {
			method: 'POST',
			body: JSON.stringify({ exchange, testnet })
		});
	}
}

export const apiClient = new ApiClient();
