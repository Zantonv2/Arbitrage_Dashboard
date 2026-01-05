# Implementation Plan: Crypto Arbitrage Monitor

## Overview

This plan implements the crypto arbitrage monitoring desktop application in phases: project setup, HTTP/WebSocket server, core data structures, computation engine, storage, connectors, simulation, and finally the UI. Each phase builds on the previous, with property tests validating correctness at each step.

## Architecture Summary

- **Backend**: Rust + axum HTTP server with WebSocket support
- **Frontend**: Svelte + Tailwind + Vite (served as static files)
- **Database**: SQLite with sqlx
- **Communication**: REST API for queries, WebSocket for real-time signal updates

## Implementation Phases

### Phase 1: MVP (Core Arbitrage)
Basic arbitrage detection. Setting up unified connector with univesal structure for every crypto exchanger, and creating traits for specialized, starting with ByBit and BingX, gross/net profit calculation, and simple frontend display.

### Phase 2: Multi-Exchange
Add support for Hyperliquid and other potential connectors.

### Phase 3: Confidence & Sizing
Add confidence scoring, size calculator, and execution preparation.

### Phase 4: Advanced Features
Simulation engine, analytics dashboard, API key encryption, full configuration UI.

---

## Tasks

### Phase 1: MVP Tasks [MVP]

- [ ] 1. Project Setup and Core Types
  - Initialize Rust workspace with axum for HTTP server
  - Set up Svelte 5 + Vite 7 + Tailwind 4 project structure
  - Configure SQLite with sqlx
  - Add dependencies: tokio, serde, rust_decimal, uuid, chrono, axum, tower-websockets
  - Define core enums: ExchangeId, Side, OrderType, TimeInForce
  - Define core structs: Symbol, OrderBookLevel, OrderBook, Signal, Order, ExecutionInstruction
  - _Requirements: 6.1, 6.2_

- [ ] A. HTTP Server Setup
  - [ ] A.1 Implement basic axum server with route macros
  - [ ] A.2 Add static file serving for Svelte build output
  - [ ] A.3 Implement CORS for local development
  - [ ] A.4 Add graceful shutdown handling
  - _Requirements: 13.1_

- [ ] B. WebSocket Server
  - [ ] B.1 Implement WebSocket upgrade handler
  - [ ] B.2 Create broadcast channel for signal updates
  - [ ] B.3 Handle client connections/disconnections
  - [ ] B.4 Implement heartbeat for WebSocket clients
  - _Requirements: 9.1_

- [ ] 2. Normalizer Module
  - [ ] 2.1 Implement symbol mapping with configurable TOML file
    - Load mappings from config file
    - Map exchange symbols to canonical base/quote format
    - Handle unknown symbols by returning None
    - _Requirements: 2.1, 2.3, 2.6_
  - [ ] 2.2 Write property test for symbol mapping
    - **Property 5: Symbol Mapping Correctness**
    - **Property 6: Unknown Symbol Handling**
    - **Validates: Requirements 2.1, 2.3**
  - [ ] 2.3 Implement precision handling
    - Track precision per symbol per exchange
    - Use minimum precision across exchanges
    - Format quantities and prices to correct decimals
    - _Requirements: 2.4_
  - [ ] 2.4 Write property test for precision
    - **Property 7: Precision Minimum Selection**
    - **Validates: Requirements 2.4**
  - [ ] 2.5 Implement fee schedule lookup
    - Store maker/taker fees per exchange
    - Support fee tier configuration
    - _Requirements: 2.2, 2.8_
  - [ ] 2.6 Implement order book normalization
    - Convert raw order book to canonical format
    - Normalize timestamps to UTC milliseconds
    - Validate bid < ask invariant
    - _Requirements: 1.2, 2.9, 2.10_
  - [ ] 2.7 Write property tests for order book normalization
    - **Property 1: Order Book Normalization Validity**
    - **Property 9: Timestamp UTC Normalization**
    - **Property 10: Invalid Order Book Rejection**
    - **Validates: Requirements 1.2, 2.9, 2.10**
  - [ ] 2.8 Implement stablecoin equivalence groups
    - Configure equivalent stablecoins (USDT, USDC, BUSD)
    - Treat equivalents as same quote currency
    - _Requirements: 2.7_
  - [ ] 2.9 Write property test for stablecoin equivalence
    - **Property 8: Stablecoin Equivalence**
    - **Validates: Requirements 2.7**

- [ ] 3. Checkpoint - Normalizer Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Arbitrage Engine
  - [ ] 4.1 Implement order book cache with DashMap
    - Store order books keyed by (exchange, symbol)
    - Support concurrent read/write access
    - Track last update timestamp per book
    - _Requirements: 3.1_
  - [ ] 4.2 Implement order book merge for partial updates
    - Merge partial updates into existing snapshot
    - Maintain sorted order after merge
    - _Requirements: 1.6_
  - [ ] 4.3 Write property test for order book merge
    - **Property 2: Order Book Merge Consistency**
    - **Validates: Requirements 1.6**
  - [ ] 4.4 Implement gross profit calculation
    - Calculate (best_bid_sell - best_ask_buy) / best_ask_buy
    - Handle edge cases (empty books, zero prices)
    - _Requirements: 3.2_
  - [ ] 4.5 Write property test for gross profit
    - **Property 11: Gross Profit Calculation**
    - **Validates: Requirements 3.2**
  - [ ] 4.6 Implement net profit calculation
    - Subtract maker/taker fees from gross profit
    - Use normalizer fee schedules
    - _Requirements: 3.3_
  - [ ] 4.7 Write property test for net profit
    - **Property 12: Net Profit Calculation**
    - **Validates: Requirements 3.3**
  - [ ] 4.8 Implement profit threshold filtering
    - Only emit signals above configurable threshold
    - Default threshold: 0.1%
    - _Requirements: 3.4_
  - [ ] 4.9 Write property test for threshold filtering
    - **Property 13: Profit Threshold Filtering**
    - **Validates: Requirements 3.4**
  - [ ] 4.10 Implement signal deduplication
    - Track recent signals by opportunity key
    - Suppress duplicates within time window
    - Re-emit on significant profit change
    - _Requirements: 3.7, 3.11_
  - [ ] 4.11 Write property test for deduplication
    - **Property 14: Signal Deduplication**
    - **Validates: Requirements 3.7, 3.11**
  - [ ] 4.12 Implement stale order book exclusion
    - Check timestamp against max age threshold
    - Exclude stale books from computation
    - _Requirements: 3.9_
  - [ ] 4.13 Write property test for stale exclusion
    - **Property 15: Stale Order Book Exclusion**
    - **Validates: Requirements 3.9**
  - [ ] 4.14 Wire up signal broadcast channel
    - Use tokio broadcast for signal distribution
    - Publish to WebSocket clients
    - _Requirements: 3.1, B.2_

- [ ] 5. Checkpoint - Arbitrage Engine Complete
  - Ensure all tests pass, ask the user if questions arise.

### Phase 2: Multi-Exchange [PHASE 2]

- [ ] 14. Exchange Connectors (continued)
  - [ ] 14.7 Implement BingX connector
    - WebSocket connection to BingX
    - Parse order book messages
    - Handle authentication
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.7_
  - [ ] 14.8 Implement Hyperliquid connector
    - WebSocket connection to Hyperliquid
    - Parse order book messages
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.7_
  - [ ] 14.9 Implement unified rate limit handling
    - Detect rate limit responses
    - Throttle and log per exchange
    - _Requirements: 1.5_

### Phase 3: Confidence & Sizing [PHASE 3]

- [ ] 6. Confidence Scorer
  - [ ] 6.1 Implement depth scoring factor
    - Calculate available liquidity at signal price
    - Normalize to 0-1 score
    - _Requirements: 4.2_
  - [ ] 6.2 Implement volatility tracking and scoring
    - Track price variance over rolling window
    - Lower volatility = higher score
    - _Requirements: 4.3_
  - [ ] 6.3 Implement exchange reliability scoring
    - Track connection uptime and error rates
    - Higher reliability = higher score
    - _Requirements: 4.4_
  - [ ] 6.4 Implement spread stability scoring
    - Track spread consistency over time
    - More stable = higher score
    - _Requirements: 4.6_
  - [ ] 6.5 Implement freshness scoring
    - Score based on order book age
    - Fresher = higher score
    - _Requirements: 4.7_
  - [ ] 6.6 Implement weighted score composition
    - Combine factors with configurable weights
    - Ensure final score in [0, 1]
    - _Requirements: 4.1, 4.10_
  - [ ] 6.7 Write property tests for confidence scoring
    - **Property 16: Confidence Score Bounds**
    - **Property 17: Confidence Factor Composition**
    - **Validates: Requirements 4.1, 4.2, 4.3, 4.4, 4.6, 4.7, 4.10**
  - [ ] 6.8 Implement confidence threshold suppression
    - Filter signals below confidence threshold
    - Default threshold: 0.3
    - _Requirements: 4.5_
  - [ ] 6.9 Write property test for confidence suppression
    - **Property 18: Confidence Threshold Suppression**
    - **Validates: Requirements 4.5**

- [ ] 7. Size Calculator
  - [ ] 7.1 Implement order book depth analysis
    - Walk price levels to find available size at slippage
    - Calculate volume-weighted average price
    - _Requirements: 5.1, 5.8_
  - [ ] 7.2 Implement slippage calculation
    - Calculate expected slippage for given size
    - Support multiple slippage tiers
    - _Requirements: 5.8, 5.9_
  - [ ] 7.3 Write property test for slippage calculation
    - **Property 23: Slippage Calculation Accuracy**
    - **Validates: Requirements 5.8**
  - [ ] 7.4 Implement size bounds enforcement
    - Respect exchange min/max limits
    - Respect user position limits
    - Apply conservative multiplier
    - _Requirements: 5.2, 5.3, 5.5, 5.6, 5.7_
  - [ ] 7.5 Write property tests for size bounds
    - **Property 19: Size Slippage Tolerance**
    - **Property 20: Size Bound Compliance**
    - **Property 21: Size Limiting Factor**
    - **Property 22: Insufficient Depth Handling**
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7**

- [ ] 8. Checkpoint - Scoring and Sizing Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 9. Execution Preparer
  - [ ] 9.1 Implement order instruction generation
    - Build buy and sell orders from signal
    - Include all required fields
    - Generate unique client order IDs
    - _Requirements: 6.1, 6.2, 6.10_
  - [ ] 9.2 Write property tests for order generation
    - **Property 24: Order Instruction Completeness**
    - **Property 29: Client Order ID Uniqueness**
    - **Validates: Requirements 6.2, 6.10**
  - [ ] 9.3 Implement slippage buffer application
    - Adjust buy price up by buffer
    - Adjust sell price down by buffer
    - _Requirements: 6.3_
  - [ ] 9.4 Write property test for slippage buffer
    - **Property 25: Slippage Buffer Application**
    - **Validates: Requirements 6.3**
  - [ ] 9.5 Implement order validation
    - Check minimum notional requirements
    - Check quantity precision
    - Return detailed errors on failure
    - _Requirements: 6.4, 6.5, 6.6_
  - [ ] 9.6 Write property tests for validation
    - **Property 26: Minimum Notional Validation**
    - **Property 27: Quantity Precision Formatting**
    - **Validates: Requirements 6.4, 6.6**
  - [ ] 9.7 Implement fee calculation
    - Calculate expected fee per order
    - Use exchange fee schedules
    - _Requirements: 6.8_
  - [ ] 9.8 Write property test for fee calculation
    - **Property 28: Fee Calculation Accuracy**
    - **Validates: Requirements 6.8**
  - [ ] 9.9 Implement execution preview
    - Calculate expected and worst-case outcomes
    - _Requirements: 6.9_

- [ ] 10. Checkpoint - Execution Preparer Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 11. Storage Service
  - [ ] 11.1 Create SQLite schema and migrations
    - signals table with indexes
    - executions table with foreign key
    - audit_log table
    - _Requirements: 7.1, 7.2, 7.8_
  - [ ] 11.2 Implement signal persistence
    - Serialize signal to JSON
    - Insert into database
    - _Requirements: 7.1, 7.5_
  - [ ] 11.3 Implement execution persistence
    - Serialize execution to JSON
    - Maintain referential integrity
    - _Requirements: 7.2, 7.11_
  - [ ] 11.4 Write property tests for storage round-trip
    - **Property 30: Signal Storage Round-Trip**
    - **Property 31: Execution Storage Round-Trip**
    - **Property 34: Referential Integrity**
    - **Validates: Requirements 7.1, 7.2, 7.5, 7.6, 7.11**
  - [ ] 11.5 Implement query with filters
    - Support date range, symbol, exchange, profit filters
    - Implement pagination
    - _Requirements: 7.3, 7.7_
  - [ ] 11.6 Write property tests for queries
    - **Property 32: Query Filter Correctness**
    - **Property 33: Pagination Consistency**
    - **Validates: Requirements 7.3, 7.7**
  - [ ] 11.7 Implement data retention cleanup
    - Delete data older than retention period
    - Run on schedule
    - _Requirements: 7.4, 7.10_
  - [ ] 11.8 Implement CSV export
    - Export filtered data to CSV format
    - _Requirements: 7.9_

- [ ] 12. Key Store
  - [ ] 12.1 Implement AES-256-GCM encryption
    - Use ring crate for crypto
    - Derive key from password with PBKDF2
    - _Requirements: 11.1, 11.6_
  - [ ] 12.2 Implement credential storage and retrieval
    - Encrypt on store, decrypt on retrieve
    - Store in platform-appropriate location
    - _Requirements: 11.1, 11.7_
  - [ ] 12.3 Write property tests for encryption
    - **Property 39: Encryption Round-Trip**
    - **Property 40: Wrong Password Rejection**
    - **Validates: Requirements 11.1, 11.3, 11.4**
  - [ ] 12.4 Implement key rotation
    - Re-encrypt all credentials with new key
    - _Requirements: 11.5_
  - [ ] 12.5 Write property test for key rotation
    - **Property 41: Key Rotation Preservation**
    - **Validates: Requirements 11.5**
  - [ ] 12.6 Implement memory clearing on lock
    - Zero sensitive data on lock/close
    - _Requirements: 11.10, 11.11_

- [ ] 13. Checkpoint - Storage and Security Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 14. Exchange Connectors
  - [ ] 14.1 Define ExchangeConnector trait
    - connect, disconnect, subscribe, unsubscribe, status methods
    - _Requirements: 1.1, 1.9, 1.10_
  - [ ] 14.2 Implement exponential backoff utility
    - Calculate delay based on attempt number
    - Cap at maximum delay
    - _Requirements: 1.3_
  - [ ] 14.3 Write property test for backoff
    - **Property 3: Exponential Backoff Bounds**
    - **Validates: Requirements 1.3**
  - [ ] 14.4 Implement sequence gap detection
    - Track expected sequence numbers
    - Detect gaps and trigger resync
    - _Requirements: 1.8_
  - [ ] 14.5 Write property test for sequence detection
    - **Property 4: Sequence Gap Detection**
    - **Validates: Requirements 1.8**
  - [ ] 14.6 Implement ByBit connector
    - WebSocket connection to ByBit
    - Parse order book messages
    - Handle reconnection
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.7_

- [ ] 15. Simulation Engine
  - [ ] 15.1 Implement historical data replay
    - Accept order book stream
    - Process in timestamp order
    - _Requirements: 8.1, 8.2_
  - [ ] 15.2 Write property test for chronological order
    - **Property 35: Simulation Chronological Order**
    - **Validates: Requirements 8.2**
  - [ ] 15.3 Wire simulation to arbitrage engine
    - Use same computation logic as live
    - _Requirements: 8.3_
  - [ ] 15.4 Write property test for logic equivalence
    - **Property 36: Simulation Logic Equivalence**
    - **Validates: Requirements 8.3**
  - [ ] 15.5 Implement PnL tracking
    - Track simulated trades and outcomes
    - Calculate running PnL
    - _Requirements: 8.4_
  - [ ] 15.6 Write property test for PnL calculation
    - **Property 37: PnL Calculation Accuracy**
    - **Validates: Requirements 8.4**
  - [ ] 15.7 Implement statistics calculation
    - Win rate, max drawdown, Sharpe ratio
    - Equity curve generation
    - _Requirements: 8.5_
  - [ ] 15.8 Write property test for statistics
    - **Property 38: Simulation Statistics Accuracy**
    - **Validates: Requirements 8.5**
  - [ ] 15.9 Implement fill models
    - Optimistic, realistic, pessimistic modes
    - Model execution delays
    - _Requirements: 8.9_
  - [ ] 15.10 Implement parameter sweep
    - Run multiple simulations with different params
    - Collect and compare results
    - _Requirements: 8.8_
  - [ ] 15.11 Implement synthetic data generation
    - Generate valid order books for stress testing
    - _Requirements: 8.12_

- [ ] 16. Checkpoint - Backend Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 17. Configuration Manager
  - [ ] 17.1 Implement TOML config loading
    - Load from .config directory
    - Parse and validate structure
    - _Requirements: 13.1, 13.3_
  - [ ] 17.2 Write property test for config validation
    - **Property 42: Config Validation**
    - **Validates: Requirements 13.3, 13.4**
  - [ ] 17.3 Implement config hot-reload
    - Watch config file for changes
    - Reload non-critical settings
    - _Requirements: 13.2_
  - [ ] 17.4 Implement environment variable overrides
    - Override config values from env vars
    - _Requirements: 13.5_

- [ ] 18. Audit Logger
  - [ ] 18.1 Implement structured logging
    - Log to file with rotation
    - Include correlation IDs
    - Sanitize sensitive data
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5, 12.6, 12.8, 12.9_
  - [ ] 18.2 Implement log streaming for UI
    - Stream logs via WebSocket
    - _Requirements: 12.10_

- [ ] 19. REST API + WebSocket Commands
  - [ ] 19.1 Implement signal query endpoint (GET /api/signals)
    - Support filter and limit query params
    - _Requirements: 9.3, 9.4_
  - [ ] 19.2 Implement execution endpoints
    - POST /api/executions/prepare
    - POST /api/executions/confirm
    - _Requirements: 6.1, 9.11_
  - [ ] 19.3 Implement order book query endpoint
    - GET /api/orderbooks/:exchange/:symbol
    - _Requirements: 9.5_
  - [ ] 19.4 Implement analytics endpoints
    - GET /api/analytics with period parameter
    - _Requirements: 10.1, 10.2, 10.3, 10.4_
  - [ ] 19.5 Write property tests for analytics calculations
    - **Property 43: Analytics Win Rate Calculation**
    - **Property 44: Analytics PnL Aggregation**
    - **Validates: Requirements 10.1, 10.4**
  - [ ] 19.6 Implement simulation endpoint
    - POST /api/simulation/run
    - _Requirements: 8.1_
  - [ ] 19.7 Implement config endpoints
    - GET /api/config
    - PUT /api/config
    - _Requirements: 13.6_
  - [ ] 19.8 Implement status endpoint
    - GET /api/status/exchanges
    - _Requirements: 1.9, 9.8_
  - [ ] 19.9 Wire signal broadcaster to WebSocket
    - Emit signals to connected frontend clients
    - _Requirements: 9.1, B.2_
  - [ ] 19.10 Implement request validation
    - Validate query params and body payloads
    - Return proper error responses

- [ ] 20. Checkpoint - API Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 21. Frontend Dashboard
  - [ ] 21.1 Create Svelte project structure
    - Set up routing, stores, components
    - Configure Tailwind CSS
    - Configure Vite proxy for API calls
    - _Requirements: 9.1_
  - [ ] 21.2 Implement signal table component
    - Display signals with all columns
    - Support sorting by any column
    - _Requirements: 9.2, 9.3_
  - [ ] 21.3 Implement signal filtering
    - Filter by asset, exchange, profit, confidence
    - _Requirements: 9.4_
  - [ ] 21.4 Implement order book visualization
    - Show depth chart for selected signal
    - _Requirements: 9.5_
  - [ ] 21.5 Implement execution panel
    - One-click prepare execution
    - Show preview before confirm
    - _Requirements: 9.11, 6.9_
  - [ ] 21.6 Implement connection status indicators
    - Show status per exchange
    - _Requirements: 9.8_
  - [ ] 21.7 Implement real-time signal updates
    - Connect to WebSocket endpoint
    - Update store reactively
    - _Requirements: 9.1, 9.7_
  - [ ] 21.8 Implement keyboard shortcuts
    - Common actions via keyboard
    - _Requirements: 9.9_
  - [ ] 21.9 Implement preference persistence
    - Save sort/filter preferences to localStorage
    - _Requirements: 9.10_

- [ ] 22. Frontend Analytics
  - [ ] 22.1 Implement PnL chart
    - Line chart with time period selection
    - _Requirements: 10.1_
  - [ ] 22.2 Implement confidence distribution chart
    - Histogram of confidence scores
    - _Requirements: 10.2_
  - [ ] 22.3 Implement exchange performance view
    - Per-exchange metrics table
    - _Requirements: 10.3_
  - [ ] 22.4 Implement summary statistics
    - Win rate, average profit display
    - _Requirements: 10.4_
  - [ ] 22.5 Implement date range selector
    - Filter all charts by date range
    - _Requirements: 10.12_

- [ ] 23. Frontend Simulation
  - [ ] 23.1 Implement simulation parameter form
    - Input fields for all params
    - _Requirements: 8.1_
  - [ ] 23.2 Implement simulation results display
    - Show statistics and equity curve
    - _Requirements: 8.5, 8.10_
  - [ ] 23.3 Implement playback controls
    - Start, pause, speed controls
    - _Requirements: 8.6, 8.7_

- [ ] 24. Frontend Configuration
  - [ ] 24.1 Implement config editor
    - Form for common settings
    - _Requirements: 13.6_
  - [ ] 24.2 Implement API key management
    - Add/remove/rotate keys
    - Never display full keys
    - _Requirements: 11.2, 11.5, 11.8_

- [ ] 25. Error Handling Integration
  - [ ] 25.1 Implement circuit breaker for exchanges
    - Track failures, open/close circuit
    - _Requirements: 14.5_
  - [ ] 25.2 Implement graceful degradation
    - Continue with available exchanges
    - _Requirements: 14.1, 14.6_
  - [ ] 25.3 Implement browser notifications
    - Alert on critical signals
    - _Requirements: 14.4_
  - [ ] 25.4 Implement UI reconnection handling
    - Show indicator, auto-reconnect WebSocket
    - _Requirements: 14.3_

- [ ] 26. Final Integration
  - [ ] 26.1 Build Svelte frontend to static files
  - [ ] 26.2 Configure axum to serve static files
  - [ ] 26.3 Create startup script
  - [ ] 26.4 Run full integration test
  - [ ] 26.5 Ensure all requirements are covered
  - [ ] 26.6 Ask user if questions arise

---

## Future / Advanced Implementation

The following features are planned but deferred to future iterations. All design and requirements documentation is preserved here for future reference.

### Triangular Arbitrage
- Direct arbitrage only in MVP (same asset, different exchanges)
- Triangular arbitrage (A→B→C→A on single exchange) requires:
  - Path-finding algorithm for cycle detection
  - Multiple order book subscriptions per exchange
  - Complex fee calculation across multiple legs
  - Higher capital requirements analysis

### Advanced Confidence Scoring
- ML-based learning from historical outcomes (Req 4.9)
- Weighted factor composition with configurable weights
- Volatility tracking over rolling windows
- Exchange reliability scoring based on uptime/error rates
- Spread stability and freshness scoring

### Enhanced Size Calculator
- Multi-tier slippage calculation
- Exchange-specific min/max limit enforcement
- User position limits
- Conservative multipliers for risk management

### Simulation Engine (Full)
- Historical data replay with timestamp ordering
- PnL tracking and running calculations
- Statistics: win rate, max drawdown, Sharpe ratio
- Equity curve generation
- Fill models: optimistic, realistic, pessimistic
- Parameter sweep for strategy optimization
- Synthetic data generation for stress testing

### Analytics Dashboard
- PnL line chart with time period selection
- Confidence distribution histogram
- Per-exchange performance metrics
- Summary statistics display
- Date range filtering

### Security Features
- AES-256-GCM encryption for API keys (Req 11.1)
- PBKDF2 key derivation
- Credential storage/retrieval
- Key rotation support
- Memory clearing on lock

### Additional Exchange Connectors
- BingX integration
- Hyperliquid integration
- Unified rate limit handling

### Advanced Configuration
- TOML config loading with hot-reload
- Environment variable overrides
- Full config UI in frontend
- API key management UI

### Observability
- Structured logging with correlation IDs
- Log rotation and sanitization
- Log streaming to frontend via WebSocket
- Circuit breaker for exchanges
- Graceful degradation handling

---

## Notes

- All tasks including property-based tests are required for comprehensive coverage
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties from the design document
- Exchange connectors can be implemented incrementally (start with ByBit, add BingX, Hyperliquid)
- Use proptest crate for Rust property-based testing
- Frontend development uses Vite dev server with hot module replacement
- Production build serves static files from Rust HTTP server
