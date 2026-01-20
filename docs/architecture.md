# Architecture

## Crate Structure

```
Arbitrage_Dashboard/
├── arbitrage-core/          # Core trading logic
│   ├── src/
│   │   ├── types/           # ExchangeId, Symbol, OrderBook, etc.
│   │   ├── strategies/      # 10 arbitrage strategy implementations
│   │   ├── confidence_scorer.rs
│   │   ├── normalizer.rs
│   │   ├── size_calculator.rs
│   │   └── test_utils/      # Test fixtures and helpers
│   └── benches/             # Criterion benchmarks
├── exchange-connectors/     # Exchange API implementations
│   ├── src/
│   │   ├── connections/     # OKX, ByBit, MEXC, Gate.io, Kraken, Bitstamp
│   │   ├── connector_trait.rs
│   │   ├── exchange_manager.rs
│   │   ├── websocket_pool.rs
│   │   ├── rate_limiter.rs
│   │   └── mock.rs          # MockConnector for testing
│   └── tests/
├── arbitrage-server/        # HTTP/WebSocket API server
│   └── tests/
├── config/                  # Configuration files
└── scripts/                 # Build and deployment scripts
```

## Key Abstractions

### ExchangeConnector Trait

```rust
trait ExchangeConnector: Send + Sync {
    fn exchange_id(&self) -> ExchangeId;
    fn config(&self) -> &ConnectorConfig;
    async fn connect(&mut self) -> Result<()>;
    async fn subscribe_order_book(&mut self, symbols: &[Symbol]) -> Result<()>;
    async fn get_order_book(&self, symbol: &Symbol) -> Result<OrderBook>;
}
```

Every exchange connector implements this trait. The exchange_manager maintains a collection of connectors and handles lifecycle.

### Strategy Trait

```rust
trait Strategy: Send + Sync {
    fn id(&self) -> StrategyId;
    fn detect(&self, market: &MarketBundle) -> Option<Signal>;
    fn filter(&self, signal: RawSignal, ctx: &FilterContext) -> Option<Signal>;
}
```

Strategies detect arbitrage opportunities from market data and return signals.

### Order Execution

```
Signal → ExecutionPreparer → OrderRequest → ExchangeConnector → Result
```

The execution_preparer validates profit, size, and risk before creating orders.

## Data Flow

1. Exchange connectors stream order books via WebSocket
2. MarketBundle aggregates data from multiple exchanges
3. Strategies scan MarketBundle for opportunities
4. Confidence scorer rates each signal
5. Execution preparer validates and creates orders
6. Order executor sends orders and handles rollbacks

## Concurrency Model

- **Exchange Connectors**: Async tokio tasks, one per exchange
- **WebSocket Pool**: Shared across connectors with auto-reconnect
- **Exchange Manager**: DashMap for lock-free connector access
- **Order Execution**: Single-threaded execution with rollback

## Error Handling

All errors propagate through `ArbitrageError` enum:

```rust
pub enum ArbitrageError {
    Exchange { exchange: ExchangeId, operation: String, details: String },
    Strategy { strategy: StrategyId, reason: String },
    Execution { side: Side, reason: String },
    Config(String),
}
```

Errors are logged with context and returned to callers for handling.
