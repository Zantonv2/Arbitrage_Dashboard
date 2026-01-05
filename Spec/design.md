# Design Document: Crypto Arbitrage Monitor

## Overview

A desktop application for real-time cryptocurrency arbitrage monitoring. The system connects to multiple exchanges via WebSocket, normalizes order book data, computes arbitrage opportunities with confidence scoring, and assists users with pre-filled trade execution. Built with Rust backend, served via local HTTP server, with a Svelte + Tailwind web UI.

### Design Goals

1. **Safety**: No autonomous trading - all executions require explicit user confirmation
2. **Performance**: Sub-100ms signal computation, sub-500ms UI updates
3. **Resilience**: Continue operating when individual exchanges fail
4. **Extensibility**: Adding new exchanges requires only implementing a connector trait
5. **Testability**: Pure computation functions separated from I/O for property-based testing

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Local Browser                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Svelte + Tailwind UI                   │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐  │   │
│  │  │Dashboard │  │Analytics │  │Simulation│  │ Config  │  │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └─────────┘  │   │
│  └─────────────────────────────────────────────────────────┘   │
│           ▲                         │                           │
│           │ HTTP / WS               │                           │
│  ┌────────┴────────┐   ┌────────────┴────────────────────────┐ │
│  │   HTTP Server   │   │           Rust Backend              │ │
│  │  (axum/warp)    │   │  ┌────────────┐    ┌────────────┐   │ │
│  └─────────────────┘   │  │ Exchange   │───▶│ Normalizer │   │ │
│          │             │  │ Connectors │    │            │   │ │
│          ▼             │  └────────────┘    └────────────┘   │ │
│  ┌─────────────────┐   │         │                  │         │ │
│  │  Static Files   │   │         ▼                  ▼         │ │
│  │  (Svelte build) │   │  ┌────────────┐    ┌──────────────┐  │ │
│  └─────────────────┘   │  │  Key Store │    │  Arbitrage   │  │ │
│                        │  │ (Encrypted)│    │   Engine     │  │ │
│                        │  └────────────┘    └──────────────┘  │ │
│                        │         │                  │         │ │
│                        │         ▼                  ▼         │ │
│                        │  ┌────────────┐    ┌──────────────┐  │ │
│                        │  │  Storage   │    │  Confidence  │  │ │
│                        │  │  Service   │    │   Scorer     │  │ │
│                        │  └────────────┘    └──────────────┘  │ │
│                        │         │                  │         │ │
│                        │         ▼                  ▼         │ │
│                        │  ┌────────────┐    ┌──────────────┐  │ │
│                        │  │  SQLite    │    │    Size      │  │ │
│                        │  │  Database  │    │  Calculator  │  │ │
│                        │  └────────────┘    └──────────────┘  │ │
│                        └──────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                           │
               ┌───────────┼───────────┐
               ▼           ▼           ▼
         ┌──────────┐ ┌──────────┐ ┌──────────┐
         │ Binance  │ │ Coinbase │ │  Kraken  │
         │   API    │ │   API    │ │   API    │
         └──────────┘ └──────────┘ └──────────┘
```

### Component Responsibilities

| Component | Responsibility | Input | Output |
|-----------|---------------|-------|--------|
| Exchange Connector | WebSocket connection management, raw data ingestion | Exchange WebSocket messages | Raw order book updates |
| Normalizer | Symbol mapping, fee lookup, precision handling | Raw order books | Canonical order books |
| Arbitrage Engine | Opportunity detection, profit calculation | Canonical order books | Raw signals |
| Confidence Scorer | Signal quality evaluation | Raw signals + market context | Scored signals |
| Size Calculator | Trade size optimization | Scored signals + order books | Size recommendations |
| Execution Preparer | Order instruction generation | Signals + user input | Validated order pairs |
| Storage Service | Persistence and retrieval | Signals, executions | Query results |
| Simulation Engine | Historical replay and testing | Historical data + params | Performance metrics |
| Key Store | Credential encryption/decryption | Master password | Decrypted credentials |
| Audit Logger | Event recording | System events | Structured logs |

### Data Flow

1. **Ingestion Flow**: Exchange → Connector → Normalizer → Order Book Cache
2. **Signal Flow**: Order Book Update → Arbitrage Engine → Confidence Scorer → Size Calculator → UI Event
3. **Execution Flow**: User Selection → Execution Preparer → Validation → User Confirmation → Storage
4. **Analytics Flow**: Storage Query → Aggregation → Chart Data → UI

### Technology Choices

| Layer | Technology | Rationale |
|-------|------------|-----------|
| Backend | Rust + axum | High-performance async HTTP, well-maintained |
| Frontend | Svelte + Tailwind + Vite | Reactive, fast, minimal bundle, great DX |
| Database | SQLite | Embedded, zero-config, sufficient for single-user |
| WebSocket | tokio-tungstenite | Async, well-maintained |
| HTTP Server | axum | Minimal, ergonomic, integrates with Tokio |
| Serialization | serde + JSON | Standard, debuggable |
| Encryption | AES-256-GCM via ring | Industry standard, audited |

## Components and Interfaces

### Exchange Connector

Manages WebSocket connections with automatic reconnection and rate limit handling.

**Responsibilities**:
- Establish and maintain WebSocket connections
- Handle authentication where required
- Parse exchange-specific message formats
- Emit connection status changes
- Implement exponential backoff for reconnection

**Key Behaviors**:
- Reconnection starts at 1s delay, doubles each attempt, caps at 60s
- Heartbeat detection triggers reconnect after 30s of silence
- Rate limit detection pauses requests and logs the event
- Partial order book updates merged with existing snapshot

**Supported Exchanges**: Binance, Coinbase, Kraken, Bybit (extensible via trait)

### Normalizer

Converts exchange-specific formats to canonical representations.

**Responsibilities**:
- Map exchange symbols to canonical base/quote format
- Look up maker/taker fees per exchange
- Apply precision rules (use minimum precision across exchanges)
- Validate order book integrity (bids < asks)
- Convert timestamps to UTC milliseconds

**Symbol Mapping Strategy**:
- Configurable mapping file for overrides
- Stablecoin groups (USDT, USDC, BUSD) treated as equivalent when configured
- Unknown symbols logged and skipped

### Arbitrage Engine

Core computation for identifying profitable opportunities.

**Responsibilities**:
- Maintain current order book state per exchange/symbol
- Compute arbitrage on every order book update
- Calculate gross and net profit including fees
- Filter signals below profit threshold
- Deduplicate signals within time window
- Track signal lifecycle (new, updated, expired)

**Profit Calculation**:
```
gross_profit = (best_bid_sell - best_ask_buy) / best_ask_buy
net_profit = gross_profit - buy_taker_fee - sell_taker_fee
```

**Signal Emission Rules**:
- Only emit if net_profit > configured threshold (default 0.1%)
- Re-emit if profit changes by > delta threshold (default 0.01%)
- Expire signals after configurable age (default 5s)

### Confidence Scorer

Evaluates signal quality using multiple weighted factors.

**Scoring Factors**:

| Factor | Weight | Calculation |
|--------|--------|-------------|
| Depth | 0.30 | Available liquidity at signal price levels |
| Volatility | 0.25 | Inverse of recent price variance |
| Reliability | 0.20 | Exchange uptime and error rate |
| Spread Stability | 0.15 | Consistency of spread over time |
| Freshness | 0.10 | Inverse of order book age |

**Output**: Score between 0.0 and 1.0, with breakdown available for UI display.

**Suppression**: Signals with confidence below threshold (default 0.3) are not emitted.

### Size Calculator

Determines optimal trade size based on order book depth and slippage tolerance.

**Algorithm**:
1. Walk order book levels until cumulative slippage exceeds tolerance
2. Take minimum of buy-side and sell-side available size
3. Apply conservative multiplier (default 0.8) for market movement buffer
4. Validate against exchange min/max limits
5. Return zero if constraints cannot be satisfied

**Slippage Tiers**: Provides recommendations at 0.05%, 0.1%, and 0.2% slippage levels.

### Execution Preparer

Generates validated order instructions for user execution.

**Responsibilities**:
- Build buy and sell order objects with all required fields
- Apply slippage buffer to limit prices
- Format quantities to exchange precision
- Validate against exchange minimums (quantity, notional)
- Generate unique client order IDs
- Calculate expected fees and preview outcomes

**Order Defaults**:
- Order type: Limit
- Time in force: IOC (Immediate or Cancel)
- Slippage buffer: 0.05%

### Storage Service

Persists signals and executions to SQLite.

**Tables**:
- `signals`: All emitted signals with full details
- `executions`: Prepared and confirmed execution instructions
- `audit_log`: System events for debugging

**Query Capabilities**:
- Filter by date range, symbol, exchange, profit range, confidence
- Pagination with configurable page size
- CSV export for external analysis

**Retention**: Automatic cleanup of data older than configured period (default 90 days).

### Simulation Engine

Replays historical data for strategy validation.

**Modes**:
- **Replay**: Process historical order books chronologically
- **Sweep**: Test parameter combinations
- **Synthetic**: Generate artificial data for stress testing

**Fill Models**:
- Optimistic: Always fill at expected price
- Realistic: Model slippage and partial fills
- Pessimistic: Assume worst-case execution

**Output Metrics**: Total PnL, win rate, max drawdown, Sharpe ratio, equity curve.

### Key Store

Secure credential storage using AES-256-GCM encryption.

**Security Measures**:
- Master password required on startup
- Key derivation via PBKDF2 (100,000 iterations minimum)
- Keys never exposed to frontend or logs
- Memory cleared on lock/close
- Platform-appropriate secure storage location

### Audit Logger

Comprehensive event logging for traceability.

**Log Categories**: Connection, Signal, Execution, Error, Performance

**Features**:
- Configurable log levels (DEBUG, INFO, WARN, ERROR)
- Log rotation by size and count
- Correlation IDs for tracing related events
- Sensitive data sanitization
- Real-time streaming to UI for debugging

## Data Models

### Order Book
- Exchange identifier
- Symbol (base/quote)
- Bids: list of (price, quantity) sorted descending
- Asks: list of (price, quantity) sorted ascending
- Timestamp (UTC milliseconds)
- Optional sequence number

### Signal
- Unique ID
- Symbol
- Buy exchange, sell exchange
- Buy price, sell price
- Gross profit %, net profit %, net profit amount
- Confidence score
- Recommended size
- Expected slippage
- Timestamp, expiration time
- Lifecycle state (new/updated/expired)

### Execution Instruction
- Unique ID
- Signal ID reference
- Buy order details
- Sell order details
- Slippage buffer
- Status (prepared/confirmed/expired/failed)
- Timestamps (created, confirmed, completed)
- Actual profit (if completed)

### Order
- Client order ID
- Exchange
- Symbol
- Side (buy/sell)
- Order type (limit/market)
- Price, quantity
- Time in force
- Expected fee

## Error Handling

### Failure Modes and Recovery

| Failure | Detection | Recovery |
|---------|-----------|----------|
| Exchange disconnect | Heartbeat timeout | Exponential backoff reconnect |
| Rate limit | HTTP 429 / WS message | Pause and resume after cooldown |
| Invalid data | Validation failure | Log, skip, continue |
| Database write failure | SQLite error | Queue for retry |
| HTTP server crash | Process exit | User restarts app |
| Crash | Process exit | Restore last known state on restart |

### Circuit Breaker Pattern
- Track failure count per exchange
- Open circuit after N consecutive failures
- Half-open after cooldown period
- Close on successful request

## Testing Strategy

### Unit Tests
- Pure computation functions (profit calculation, confidence scoring, size calculation)
- Data normalization and validation
- Serialization/deserialization round-trips

### Property-Based Tests
- Arbitrage profit calculation invariants
- Order book normalization consistency
- Signal serialization round-trips
- Size calculation bounds

### Integration Tests
- Exchange connector with mock WebSocket server
- Storage service with test database
- End-to-end signal flow

### Simulation Tests
- Known historical scenarios with expected outcomes
- Edge cases (empty books, extreme spreads, rapid updates)



## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Order Book Normalization Validity

*For any* raw order book from any supported exchange, normalizing it SHALL produce a canonical order book where bids are sorted descending by price, asks are sorted ascending by price, and all prices/quantities use the minimum precision.

**Validates: Requirements 1.2, 2.1, 2.4**

### Property 2: Order Book Merge Consistency

*For any* existing order book snapshot and any partial update, merging them SHALL produce an order book where the update's levels replace or add to the snapshot's levels, maintaining sorted order and no duplicate price levels.

**Validates: Requirements 1.6**

### Property 3: Exponential Backoff Bounds

*For any* reconnection attempt number N, the backoff delay SHALL equal min(initial_delay * 2^(N-1), max_delay), where initial_delay=1s and max_delay=60s.

**Validates: Requirements 1.3**

### Property 4: Sequence Gap Detection

*For any* sequence of messages with sequence numbers, if there exists a gap (missing number), the system SHALL detect it and flag for resync.

**Validates: Requirements 1.8**

### Property 5: Symbol Mapping Correctness

*For any* exchange symbol with a known mapping, normalization SHALL produce the exact canonical base/quote format defined in the mapping configuration.

**Validates: Requirements 2.1**

### Property 6: Unknown Symbol Handling

*For any* exchange symbol without a known mapping, normalization SHALL return None/skip and not produce a canonical symbol.

**Validates: Requirements 2.3**

### Property 7: Precision Minimum Selection

*For any* symbol traded on multiple exchanges with different precisions, the normalized precision SHALL equal the minimum precision across all exchanges for that symbol.

**Validates: Requirements 2.4**

### Property 8: Stablecoin Equivalence

*For any* two stablecoins configured in the same equivalence group, they SHALL be treated as interchangeable for arbitrage computation purposes.

**Validates: Requirements 2.7**

### Property 9: Timestamp UTC Normalization

*For any* timestamp input in any format, normalization SHALL produce a UTC timestamp in milliseconds since Unix epoch.

**Validates: Requirements 2.9**

### Property 10: Invalid Order Book Rejection

*For any* order book where best_bid_price >= best_ask_price, validation SHALL reject the order book as invalid.

**Validates: Requirements 2.10**

### Property 11: Gross Profit Calculation

*For any* two order books for the same symbol on different exchanges, gross_profit SHALL equal (best_bid_sell - best_ask_buy) / best_ask_buy, where best_bid_sell is from the selling exchange and best_ask_buy is from the buying exchange.

**Validates: Requirements 3.2**

### Property 12: Net Profit Calculation

*For any* gross profit and fee schedules for buy and sell exchanges, net_profit SHALL equal gross_profit - buy_taker_fee - sell_taker_fee.

**Validates: Requirements 3.3**

### Property 13: Profit Threshold Filtering

*For any* computed signal where net_profit < configured_threshold, the signal SHALL NOT be emitted to subscribers.

**Validates: Requirements 3.4**

### Property 14: Signal Deduplication

*For any* sequence of signals for the same opportunity (same symbol, buy exchange, sell exchange) within the deduplication window, only the first signal SHALL be emitted; subsequent signals SHALL be suppressed unless profit changes by more than delta_threshold.

**Validates: Requirements 3.7, 3.11**

### Property 15: Stale Order Book Exclusion

*For any* order book with timestamp older than (current_time - max_age_threshold), it SHALL be excluded from arbitrage computation.

**Validates: Requirements 3.9**

### Property 16: Confidence Score Bounds

*For any* signal and scoring context, the computed confidence score SHALL be in the range [0.0, 1.0] inclusive.

**Validates: Requirements 4.1**

### Property 17: Confidence Factor Composition

*For any* confidence score, the weighted sum of individual factor scores (depth, volatility, reliability, spread_stability, freshness) multiplied by their respective weights SHALL equal the final confidence score.

**Validates: Requirements 4.2, 4.3, 4.4, 4.6, 4.7, 4.10**

### Property 18: Confidence Threshold Suppression

*For any* signal with confidence score < configured_threshold, the signal SHALL NOT be emitted to subscribers.

**Validates: Requirements 4.5**

### Property 19: Size Slippage Tolerance

*For any* recommended trade size, executing that size against the order book SHALL result in actual slippage <= configured_slippage_tolerance.

**Validates: Requirements 5.1**

### Property 20: Size Bound Compliance

*For any* recommended trade size, it SHALL satisfy: (size >= exchange_min OR size == 0) AND size <= exchange_max AND size <= user_max_position.

**Validates: Requirements 5.5, 5.6, 5.7**

### Property 21: Size Limiting Factor

*For any* size recommendation, the recommended_size SHALL be <= min(buy_side_available, sell_side_available) * conservative_multiplier.

**Validates: Requirements 5.2, 5.3**

### Property 22: Insufficient Depth Handling

*For any* order book pair where available depth < exchange_minimum_size, the size recommendation SHALL be zero.

**Validates: Requirements 5.4**

### Property 23: Slippage Calculation Accuracy

*For any* order book and trade size, the calculated expected slippage SHALL equal the volume-weighted average price deviation from the best price for that size.

**Validates: Requirements 5.8**

### Property 24: Order Instruction Completeness

*For any* generated order instruction, both buy_order and sell_order SHALL contain: exchange, symbol, side, order_type, price, quantity, time_in_force, and expected_fee.

**Validates: Requirements 6.2**

### Property 25: Slippage Buffer Application

*For any* execution instruction, buy_order.price SHALL equal signal.buy_price * (1 + slippage_buffer) and sell_order.price SHALL equal signal.sell_price * (1 - slippage_buffer).

**Validates: Requirements 6.3**

### Property 26: Minimum Notional Validation

*For any* order where price * quantity < exchange_min_notional, validation SHALL fail with a specific error indicating the notional requirement.

**Validates: Requirements 6.4**

### Property 27: Quantity Precision Formatting

*For any* generated order, the quantity SHALL have exactly the number of decimal places specified by the exchange's precision rules for that symbol.

**Validates: Requirements 6.6**

### Property 28: Fee Calculation Accuracy

*For any* order, expected_fee SHALL equal quantity * price * exchange_fee_rate for the appropriate fee type (maker/taker).

**Validates: Requirements 6.8**

### Property 29: Client Order ID Uniqueness

*For any* set of generated order instructions, all client_order_ids SHALL be unique.

**Validates: Requirements 6.10**

### Property 30: Signal Storage Round-Trip

*For any* signal, saving it to storage and then retrieving it SHALL produce a signal with identical field values.

**Validates: Requirements 7.1, 7.5, 7.6**

### Property 31: Execution Storage Round-Trip

*For any* execution instruction, saving it to storage and then retrieving it SHALL produce an instruction with identical field values.

**Validates: Requirements 7.2, 7.5, 7.6**

### Property 32: Query Filter Correctness

*For any* query with filters, all returned results SHALL match all specified filter criteria (date range, symbol, exchange, profit range).

**Validates: Requirements 7.3**

### Property 33: Pagination Consistency

*For any* paginated query, the union of all pages SHALL equal the complete result set, with no duplicates and no missing items.

**Validates: Requirements 7.7**

### Property 34: Referential Integrity

*For any* execution instruction in storage, its signal_id SHALL reference a valid signal that exists in storage.

**Validates: Requirements 7.11**

### Property 35: Simulation Chronological Order

*For any* simulation run, order books SHALL be processed in strictly ascending timestamp order.

**Validates: Requirements 8.2**

### Property 36: Simulation Logic Equivalence

*For any* order book sequence, the signals produced by simulation SHALL be identical to signals that would be produced by the live arbitrage engine given the same inputs and configuration.

**Validates: Requirements 8.3**

### Property 37: PnL Calculation Accuracy

*For any* set of simulated trades, total_pnl SHALL equal the sum of individual trade profits minus fees.

**Validates: Requirements 8.4**

### Property 38: Simulation Statistics Accuracy

*For any* simulation result, win_rate SHALL equal (profitable_trades / total_trades), and max_drawdown SHALL equal the maximum peak-to-trough decline in the equity curve.

**Validates: Requirements 8.5**

### Property 39: Encryption Round-Trip

*For any* credentials, encrypting with a password and then decrypting with the same password SHALL produce identical credentials.

**Validates: Requirements 11.1**

### Property 40: Wrong Password Rejection

*For any* encrypted credentials, attempting to decrypt with an incorrect password SHALL fail and not reveal any credential data.

**Validates: Requirements 11.3, 11.4**

### Property 41: Key Rotation Preservation

*For any* stored credentials, after key rotation with a new password, decrypting with the new password SHALL produce the original credentials.

**Validates: Requirements 11.5**

### Property 42: Config Validation

*For any* configuration value outside its defined valid range or type, validation SHALL fail with a specific error message.

**Validates: Requirements 13.3, 13.4**

### Property 43: Analytics Win Rate Calculation

*For any* set of executed trades, win_rate SHALL equal (trades where actual_profit > 0) / total_trades.

**Validates: Requirements 10.4**

### Property 44: Analytics PnL Aggregation

*For any* time period, the displayed cumulative PnL SHALL equal the sum of all trade profits within that period.

**Validates: Requirements 10.1**
