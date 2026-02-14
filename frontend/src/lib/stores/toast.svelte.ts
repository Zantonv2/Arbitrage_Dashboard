/**
 * Toast notification store for displaying success/error messages
 */

export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface Toast {
	id: string;
	type: ToastType;
	title: string;
	message?: string;
	duration: number;
	createdAt: number;
}

class ToastStore {
	toasts = $state<Toast[]>([]);
	private defaultDuration = 5000;

	add(toast: Omit<Toast, 'id' | 'createdAt'>): string {
		const id = crypto.randomUUID();
		const newToast: Toast = {
			...toast,
			id,
			createdAt: Date.now(),
			duration: toast.duration ?? this.defaultDuration
		};
		
		this.toasts = [...this.toasts, newToast];
		
		// Auto-remove after duration
		if (newToast.duration > 0) {
			setTimeout(() => this.remove(id), newToast.duration);
		}
		
		return id;
	}

	remove(id: string): void {
		this.toasts = this.toasts.filter(t => t.id !== id);
	}

	clear(): void {
		this.toasts = [];
	}

	// Convenience methods
	success(title: string, message?: string, duration?: number): string {
		return this.add({ type: 'success', title, message, duration: duration ?? this.defaultDuration });
	}

	error(title: string, message?: string, duration?: number): string {
		return this.add({ type: 'error', title, message, duration: duration ?? this.defaultDuration });
	}

	warning(title: string, message?: string, duration?: number): string {
		return this.add({ type: 'warning', title, message, duration: duration ?? this.defaultDuration });
	}

	info(title: string, message?: string, duration?: number): string {
		return this.add({ type: 'info', title, message, duration: duration ?? this.defaultDuration });
	}
}

export const toastStore = new ToastStore();
