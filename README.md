# Arbitrage Dashboard

Real-time cryptocurrency arbitrage monitoring across 6 exchanges.

## Quick Start

```bash
git clone https://github.com/Zantonv2/Arbitrage_Dashboard.git
cd Arbitrage_Dashboard
cargo build --release
```

## What It Does

Monitors order books on OKX, ByBit, MEXC, Gate.io, Kraken, and Bitstamp for price discrepancies. When a profitable opportunity is detected, it can execute trades automatically.

## Current Status

| Metric | Value |
|--------|-------|
| Tests | 1,083 passing (100%) |
| Exchanges | 6 connected |
| Strategies | 10 implemented |
| Crates | 3 (core, connectors, server) |

### Implemented

- **Exchange Connectors**: OKX, ByBit, MEXC, Gate.io, Kraken, Bitstamp
- **Arbitrage Strategies**: CEX, funding rate, stablecoin, cross-exchange, new-listing, spot-perp, convergence, hedged funding, latency, spread capture
- **Order Execution**: Full execution with rollback on failure
- **Key Storage**: Encrypted credential management

### In Progress

- Frontend dashboard (Svelte)
- Backtesting/simulation
- Analytics and reporting

### Not Implemented

- AI/ML confidence scoring (manual thresholds only)
- Paper trading mode

## Configuration

Copy `config/config.example.toml` to `config/config.toml` and add your API keys.

## Testing

```bash
cargo test --workspace    # All tests
cargo bench              # Performance benchmarks
cargo tarpaulin --out HtmlReport  # Coverage report
```

## Architecture

```
arbitrage-core/        # Core trading logic
exchange-connectors/   # Exchange API implementations
arbitrage-server/      # HTTP/WebSocket API
```

## License

Apache-2.0
