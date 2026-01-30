import { z } from 'zod';
import {
	TradeSignalSchema,
	ExchangeStatusSchema,
	SystemStatusSchema
} from '$lib/schemas';
import type { TradeSignal, ExchangeStatus, ApiResponse } from '$lib/types';

const API_BASE_URL = '/api';

class ApiClient {
	private getAuthHeaders(): HeadersInit {
		const token = localStorage.getItem('jwt_token');
		return token ? { 'Authorization': `Bearer ${token}` } : {};
	}

	private async request<T>(
		endpoint: string,
		options?: RequestInit,
		schema?: z.ZodSchema<T>
	): Promise<T> {
		const response = await fetch(`${API_BASE_URL}${endpoint}`, {
			...options,
			headers: {
				'Content-Type': 'application/json',
				...this.getAuthHeaders(),
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

	// Authentication
	async login(username: string, password: string): Promise<{
		success: boolean;
		token?: string;
		expires_at?: string;
		message: string;
	}> {
		const response = await fetch(`${API_BASE_URL}/auth/login`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ username, password })
		});
		return response.json();
	}
}

export const apiClient = new ApiClient();
