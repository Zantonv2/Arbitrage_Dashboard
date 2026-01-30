import type { WebSocketMessage } from '$lib/types';

export class WebSocketClient {
	private ws: WebSocket | null = $state(null);
	private reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
	private reconnectAttempts = 0;
	private maxReconnectAttempts = 10;
	private baseReconnectDelay = 1000;

	public connectionStatus = $state<'connecting' | 'connected' | 'disconnected'>('disconnected');
	public lastError = $state<string | null>(null);

	private messageHandlers: Set<(message: WebSocketMessage) => void> = new Set();

	constructor(private url: string) {}

	connect(): void {
		if (this.ws?.readyState === WebSocket.OPEN) return;

		this.connectionStatus = 'connecting';
		
		// Get JWT token from localStorage
		const token = localStorage.getItem('jwt_token');
		const wsUrl = token ? `${this.url}?token=${token}` : this.url;
		
		this.ws = new WebSocket(wsUrl);

		this.ws.onopen = () => {
			this.connectionStatus = 'connected';
			this.reconnectAttempts = 0;
			this.lastError = null;
			console.log('WebSocket connected');
		};

		this.ws.onmessage = (event) => {
			try {
				const message: WebSocketMessage = JSON.parse(event.data);
				this.messageHandlers.forEach((handler) => handler(message));
			} catch (error) {
				console.error('Failed to parse WebSocket message:', error);
				this.lastError = 'Invalid message format';
			}
		};

		this.ws.onerror = (error) => {
			console.error('WebSocket error:', error);
			this.lastError = 'Connection error';
		};

		this.ws.onclose = () => {
			this.connectionStatus = 'disconnected';
			console.log('WebSocket disconnected');
			this.scheduleReconnect();
		};
	}

	private scheduleReconnect(): void {
		if (this.reconnectAttempts >= this.maxReconnectAttempts) {
			this.lastError = 'Max reconnection attempts reached';
			return;
		}

		const delay = Math.min(
			this.baseReconnectDelay * Math.pow(2, this.reconnectAttempts),
			30000
		);
		this.reconnectAttempts++;

		this.reconnectTimeout = setTimeout(() => {
			console.log(`Reconnecting... (attempt ${this.reconnectAttempts})`);
			this.connect();
		}, delay);
	}

	disconnect(): void {
		if (this.reconnectTimeout) {
			clearTimeout(this.reconnectTimeout);
			this.reconnectTimeout = null;
		}
		if (this.ws) {
			this.ws.close();
			this.ws = null;
		}
		this.connectionStatus = 'disconnected';
	}

	onMessage(handler: (message: WebSocketMessage) => void): () => void {
		this.messageHandlers.add(handler);
		return () => this.messageHandlers.delete(handler);
	}

	send(data: unknown): void {
		if (this.ws?.readyState === WebSocket.OPEN) {
			this.ws.send(JSON.stringify(data));
		} else {
			console.warn('WebSocket not connected, cannot send message');
		}
	}
}
