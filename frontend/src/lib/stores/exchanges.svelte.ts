import type { ExchangeStatus, ExchangeId } from '$lib/types';

class ExchangesStore {
	exchanges = $state<Map<ExchangeId, ExchangeStatus>>(new Map());

	updateExchange(status: ExchangeStatus): void {
		this.exchanges.set(status.exchange, status);
		this.exchanges = new Map(this.exchanges);
	}

	getExchange(id: ExchangeId): ExchangeStatus | undefined {
		return this.exchanges.get(id);
	}

	allExchanges = $derived(Array.from(this.exchanges.values()));

	onlineExchanges = $derived(
		this.allExchanges.filter((e) => e.status === 'online')
	);

	offlineExchanges = $derived(
		this.allExchanges.filter((e) => e.status === 'offline')
	);
}

export const exchangesStore = new ExchangesStore();
