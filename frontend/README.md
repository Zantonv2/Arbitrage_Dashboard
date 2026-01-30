# Arbitrage Dashboard Frontend

Modern, accessible trading dashboard built with SvelteKit 2.0, Svelte 5, and Tailwind CSS 4.

## Tech Stack

- **Framework**: SvelteKit 2.0+ with Svelte 5 runes
- **Runtime**: Bun 1.0+
- **Styling**: Tailwind CSS 4+
- **Types**: TypeScript 5+ (strict mode)
- **Validation**: Zod for API/WebSocket data
- **Testing**: Vitest (unit) + Playwright (E2E)

## Features

- ✅ Real-time WebSocket updates for signals and exchange status
- ✅ Auto/Manual execution mode with confidence threshold
- ✅ 10 arbitrage strategies with enable/disable controls
- ✅ Exchange status monitoring with latency and rate limits
- ✅ Dark mode support with system preference detection
- ✅ Fully accessible (WCAG 2.1 AAA compliant)
- ✅ Desktop-first design (1024px+ minimum)

## Getting Started

### Prerequisites

- Bun 1.0+ ([installation guide](https://bun.sh/docs/installation))
- Backend server running on `http://localhost:8080`

### Installation

\`\`\`bash
cd frontend
bun install
\`\`\`

### Development

\`\`\`bash
bun run dev
\`\`\`

Open [http://localhost:5173](http://localhost:5173) in your browser.

### Build

\`\`\`bash
bun run build
bun run preview
\`\`\`

### Testing

\`\`\`bash
# Unit tests
bun run test

# E2E tests
bun run test:e2e

# Linting
bun run lint
\`\`\`

## Project Structure

\`\`\`
frontend/
├── src/
│   ├── routes/              # SvelteKit routes
│   │   ├── +layout.svelte   # Root layout with WebSocket
│   │   ├── +page.svelte     # Dashboard home
│   │   ├── signals/         # Signal feed views
│   │   ├── exchanges/       # Exchange status
│   │   └── strategies/      # Strategy configuration
│   ├── lib/
│   │   ├── components/      # Reusable components
│   │   ├── stores/          # Svelte stores (signals, exchanges, system)
│   │   ├── websocket/       # WebSocket client
│   │   ├── types/           # TypeScript interfaces
│   │   └── schemas/         # Zod validation schemas
│   ├── app.html             # HTML template
│   └── app.css              # Global styles + Tailwind
├── tests/                   # Test files
└── package.json
\`\`\`

## Key Components

### ExecutionModeToggle
- Auto/Manual mode switch with confirmation dialog
- Confidence threshold slider (50-100%)
- Emergency stop button

### StrategySelector
- 10 strategy checkboxes with enable/disable all
- Real-time strategy count display

### SignalCard
- Compact and detailed variants
- Profit/loss color coding
- Exchange flow visualization
- Confidence score display

### ExchangeCard
- Connection status indicator
- Latency monitoring
- Rate limit usage bar

## WebSocket Integration

The dashboard connects to `ws://localhost:8080/ws` and handles:

- `signal_update`: New trade signals
- `exchange_status`: Exchange connection updates
- `orderbook_update`: Order book changes
- `execution_update`: Trade execution status
- `system_status`: System configuration changes

Auto-reconnect with exponential backoff is implemented.

## Accessibility

- WCAG 2.1 AAA compliant
- Keyboard navigation support
- ARIA live regions for updates
- High contrast color palette (7:1 ratio)
- Color-blind safe palette
- 44x44px minimum click targets
- Respects `prefers-reduced-motion`

## API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | /api/signals | Fetch signals with filters |
| GET | /api/exchanges/status | Exchange status |
| POST | /api/execute | Execute signal |
| GET | /api/config | Get configuration |
| PUT | /api/config | Update configuration |

## Performance Targets

- LCP: < 2.5s
- FID: < 100ms
- WebSocket processing: < 10ms
- UI update latency: < 50ms
- Bundle size: < 500KB gzipped

## License

See LICENSE file in root directory.
