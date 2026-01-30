/** @type {import('tailwindcss').Config} */
export default {
	content: ['./src/**/*.{html,js,svelte,ts}'],
	darkMode: 'class',
	theme: {
		extend: {
			colors: {
				profit: 'oklch(0.7 0.15 142)',
				loss: 'oklch(0.6 0.2 25)',
				neutral: 'oklch(0.5 0.02 247)',
				okx: 'oklch(0.4 0.15 240)',
				bybit: 'oklch(0.45 0.2 45)',
				mexc: 'oklch(0.35 0.18 280)',
				gateio: 'oklch(0.4 0.16 200)',
				kraken: 'oklch(0.3 0.12 260)',
				bitstamp: 'oklch(0.4 0.14 120)'
			},
			fontFamily: {
				mono: ['"JetBrains Mono"', '"Fira Code"', 'monospace'],
				display: ['Inter', 'system-ui', 'sans-serif']
			},
			spacing: {
				signal: '0.75rem',
				card: '1rem',
				section: '1.5rem'
			}
		}
	},
	plugins: []
};
