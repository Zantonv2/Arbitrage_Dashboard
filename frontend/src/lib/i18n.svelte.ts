// i18n support for English and Russian using Svelte 5 runes
import en from './i18n/en.json';
import ru from './i18n/ru.json';

export type Language = 'en' | 'ru';

// Type for translations
export type Translations = typeof en;

// Translation resources
const resources: Record<Language, Translations> = {
	en,
	ru
};

// Currency symbols based on language
export const currencySymbols: Record<Language, string> = {
	en: '$',
	ru: '₽'
};

// Reactive state for current language
let currentLanguage = $state<Language>('en');

// Reactive state for translations
let translations = $state<Translations>(resources['en']);

// Get the current language
export function getLanguage(): Language {
	if (typeof window !== 'undefined') {
		const saved = localStorage.getItem('triangulum_language');
		if (saved === 'en' || saved === 'ru') {
			currentLanguage = saved;
		} else {
			const browserLang = navigator.language.split('-')[0];
			if (browserLang === 'ru') {
				currentLanguage = 'ru';
			}
		}
	}
	return currentLanguage;
}

// Set the current language and load translations
export async function setLanguage(lang: Language): Promise<void> {
	currentLanguage = lang;
	
	if (typeof window !== 'undefined') {
		localStorage.setItem('triangulum_language', lang);
		document.documentElement.lang = lang;
	}
	
	// Load translations for the selected language
	translations = resources[lang];
}

// Initialize language on module load
if (typeof window !== 'undefined') {
	getLanguage();
	translations = resources[currentLanguage];
	document.documentElement.lang = currentLanguage;
}

// Translation function with dot-notation key support
export function t(key: string): string {
	const keys = key.split('.');
	let result: unknown = translations;
	
	for (const k of keys) {
		if (result && typeof result === 'object' && k in result) {
			result = (result as Record<string, unknown>)[k];
		} else {
			// Fallback to English
			let fallback: unknown = resources['en'];
			for (const fk of keys) {
				if (fallback && typeof fallback === 'object' && fk in fallback) {
					fallback = (fallback as Record<string, unknown>)[fk];
				} else {
					return key;
				}
			}
			return typeof fallback === 'string' ? fallback : key;
		}
	}
	
	return typeof result === 'string' ? result : key;
}

// Translation function with parameters
export function tp(key: string, params: Record<string, string | number>): string {
	let text = t(key);
	
	for (const [param, value] of Object.entries(params)) {
		text = text.replace(`{${param}}`, String(value));
	}
	
	return text;
}

// Format currency based on current language
export function formatCurrency(value: number, lang?: Language): string {
	const l = lang || currentLanguage;
	const symbol = currencySymbols[l];
	
	if (l === 'ru') {
		// Russian format: spaces as thousand separators
		return `${symbol}${value.toLocaleString('ru-RU')}`;
	}
	
	return `${symbol}${value.toLocaleString('en-US')}`;
}

// Format percentage
export function formatPercent(value: number, lang?: Language): string {
	const l = lang || currentLanguage;
	
	if (l === 'ru') {
		return `${value.toLocaleString('ru-RU')}%`;
	}
	
	return `${value.toLocaleString('en-US')}%`;
}

// Get strategy name by key (with short variants)
export function getStrategyName(strategyKey: string, short: boolean = false): string {
	const shortKey = `${strategyKey}-short` as keyof Translations['strategies'];
	const fullKey = strategyKey as keyof Translations['strategies'];
	
	const strategies = translations.strategies as Record<string, string>;
	
	if (short && shortKey in strategies) {
		return strategies[shortKey];
	}
	
	return strategies[fullKey] || strategyKey;
}

// Get strategy description by key
export function getStrategyDescription(strategyKey: string): string {
	const descKey = `${strategyKey}-desc` as keyof Translations['strategies'];
	const strategies = translations.strategies as Record<string, string>;
	
	return strategies[descKey] || '';
}

// Export current translations state accessor for direct access if needed
export function getTranslations(): Translations {
	return translations;
}

// Get recommendations array for a given key
export function getRecommendations(key: string): string[] {
	const keys = key.split('.');
	let result: unknown = translations;
	
	for (const k of keys) {
		if (result && typeof result === 'object' && k in result) {
			result = (result as Record<string, unknown>)[k];
		} else {
			return [];
		}
	}
	
	return Array.isArray(result) ? result : [];
}
