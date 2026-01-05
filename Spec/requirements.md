# Requirements Document

## Introduction

A desktop application for monitoring multiple cryptocurrency exchanges in real-time, identifying arbitrage opportunities, scoring them by confidence, and assisting users with pre-filled trades for manual execution. The system includes simulation and analytics capabilities to improve decision-making. Built with a Rust backend engine, Tauri for desktop integration, and Svelte + Tailwind for the UI.

## Glossary

- **Signal**: An identified arbitrage opportunity between two exchanges with computed profit and confidence
- **Confidence**: A score (0.0-1.0) indicating the likelihood of a profitable outcome based on market conditions
- **Pre_Fill**: A recommended order with price, size, and exchange details ready for user execution
- **Order_Book**: A snapshot of bids and asks for a trading pair on an exchange
- **Order_Book_Level**: A single price/quantity pair in an order book
- **Slippage**: The difference between expected and actual execution price due to market movement
- **Slippage_Buffer**: A percentage added to limit prices to account for expected slippage
- **Arbitrage_Engine**: The component that computes arbitrage opportunities from normalized order books
- **Exchange_Connector**: A module that connects to a specific exchange's API and normalizes data
- **Simulation_Engine**: A component that allows testing strategies against historical or synthetic data
- **Normalizer**: Component that converts exchange-specific formats to canonical representations
- **Confidence_Scorer**: Component that evaluates signal quality based on multiple factors
- **Size_Calculator**: Component that determines optimal trade size based on order book depth
- **Execution_Preparer**: Component that generates validated order instructions
- **Storage_Service**: Component that persists and retrieves historical data
- **Key_Store**: Secure storage for exchange API credentials
- **Audit_Logger**: System-wide logging component for traceability
- **Spread**: The difference between best ask and best bid prices
- **Depth**: The cumulative volume available at price levels in an order book
- **Latency**: Time delay between data generation and processing
- **Backoff**: Progressive delay strategy for retry attempts

## Requirements

### Requirement 1: Exchange Data Ingestion

**User Story:** As a trader, I want to receive live order book data from multiple exchanges, so that I can identify price discrepancies across markets.

#### Acceptance Criteria

1. WHEN the application starts with configured exchanges, THE Exchange_Connector SHALL establish WebSocket connections to each exchange within 5 seconds
2. WHEN an exchange sends order book updates, THE Exchange_Connector SHALL normalize the data into a canonical Order_Book format within 200ms
3. WHEN an exchange connection fails, THE Exchange_Connector SHALL attempt reconnection with exponential backoff starting at 1 second, maxing at 60 seconds
4. WHILE connected to an exchange, THE Exchange_Connector SHALL maintain a heartbeat to detect stale connections within 30 seconds
5. IF an exchange rate limits the connection, THEN THE Exchange_Connector SHALL throttle requests and log the event with exchange name and limit details
6. WHEN receiving partial order book updates, THE Exchange_Connector SHALL merge them with the existing snapshot maintaining consistency
7. THE Exchange_Connector SHALL support at minimum: Binance, Coinbase, Kraken, and Bybit exchanges
8. WHEN an exchange provides sequence numbers, THE Exchange_Connector SHALL validate message ordering and request resync on gaps
9. THE Exchange_Connector SHALL emit connection status events for UI display
10. WHEN configured symbols change, THE Exchange_Connector SHALL subscribe/unsubscribe without full reconnection

### Requirement 2: Symbol and Fee Normalization

**User Story:** As a trader, I want symbols and fees normalized across exchanges, so that I can accurately compare prices.

#### Acceptance Criteria

1. THE Normalizer SHALL map exchange-specific symbols to a canonical format using base/quote notation (e.g., "BTC/USDT")
2. THE Normalizer SHALL include maker and taker trading fees for each exchange in profit calculations
3. WHEN a symbol mapping is unknown, THE Normalizer SHALL log a warning with exchange and original symbol, then skip the symbol
4. THE Normalizer SHALL handle precision differences between exchanges by using the minimum precision of the pair
5. THE Normalizer SHALL convert all prices to a common quote currency when necessary
6. THE Normalizer SHALL maintain a configurable symbol mapping file for custom overrides
7. THE Normalizer SHALL handle stablecoin equivalents (USDT, USDC, BUSD) as configurable groups
8. WHEN fee tiers exist, THE Normalizer SHALL use the user's configured tier or default to standard fees
9. THE Normalizer SHALL normalize timestamps to UTC milliseconds
10. THE Normalizer SHALL validate that bid prices are below ask prices, discarding invalid books

### Requirement 3: Arbitrage Signal Computation

**User Story:** As a trader, I want the system to compute arbitrage opportunities in real-time, so that I can act on profitable trades quickly.

#### Acceptance Criteria

1. WHEN order books are updated, THE Arbitrage_Engine SHALL compute potential arbitrage signals within 100ms
2. THE Arbitrage_Engine SHALL calculate gross profit as (best_bid_sell_exchange - best_ask_buy_exchange) / best_ask_buy_exchange
3. THE Arbitrage_Engine SHALL calculate net profit by subtracting maker/taker fees from both sides of the trade
4. THE Arbitrage_Engine SHALL only emit signals where net profit exceeds a configurable threshold (default 0.1%)
5. WHEN computing signals, THE Arbitrage_Engine SHALL consider order book depth to determine maximum executable size
6. THE Arbitrage_Engine SHALL compute signals for all configured symbol pairs across all exchange combinations
7. THE Arbitrage_Engine SHALL deduplicate signals for the same opportunity within a configurable time window
8. THE Arbitrage_Engine SHALL include estimated execution time based on exchange latencies
9. WHEN order books are stale (older than configurable threshold), THE Arbitrage_Engine SHALL exclude them from computation
10. THE Arbitrage_Engine SHALL compute both direct and triangular arbitrage opportunities
11. THE Arbitrage_Engine SHALL emit signal updates when profit changes by more than a configurable delta
12. THE Arbitrage_Engine SHALL track signal lifecycle (new, updated, expired) for UI state management

### Requirement 4: Confidence Scoring

**User Story:** As a trader, I want each signal scored by confidence, so that I can prioritize higher-probability opportunities.

#### Acceptance Criteria

1. THE Confidence_Scorer SHALL compute a confidence score between 0.0 and 1.0 for each signal
2. THE Confidence_Scorer SHALL factor in order book depth (deeper books = higher confidence) with configurable weight
3. THE Confidence_Scorer SHALL factor in historical volatility over the past hour (lower volatility = higher confidence)
4. THE Confidence_Scorer SHALL factor in exchange reliability metrics based on recent connection stability
5. WHEN confidence is below a configurable threshold (default 0.3), THE Arbitrage_Engine SHALL suppress the signal
6. THE Confidence_Scorer SHALL factor in spread stability (consistent spreads = higher confidence)
7. THE Confidence_Scorer SHALL factor in time since last order book update (fresher = higher confidence)
8. THE Confidence_Scorer SHALL penalize signals during high-volatility market events
9. THE Confidence_Scorer SHALL learn from historical signal outcomes to adjust factor weights
10. THE Confidence_Scorer SHALL provide a breakdown of contributing factors for each score

### Requirement 5: Trade Size Recommendation

**User Story:** As a trader, I want recommended trade sizes based on order book depth, so that I can execute without excessive slippage.

#### Acceptance Criteria

1. THE Size_Calculator SHALL recommend a trade size that can be filled within a configurable slippage tolerance (default 0.1%)
2. THE Size_Calculator SHALL analyze both buy and sell order books to find the limiting factor
3. THE Size_Calculator SHALL apply a conservative multiplier (default 0.8) to account for market movement
4. WHEN order book depth is insufficient for minimum trade size, THE Size_Calculator SHALL recommend zero and flag the signal
5. THE Size_Calculator SHALL respect exchange minimum order size requirements
6. THE Size_Calculator SHALL respect exchange maximum order size limits
7. THE Size_Calculator SHALL consider user-configured maximum position size limits
8. THE Size_Calculator SHALL calculate expected slippage at the recommended size
9. THE Size_Calculator SHALL provide size recommendations at multiple slippage tiers (0.05%, 0.1%, 0.2%)
10. THE Size_Calculator SHALL account for partial fill scenarios in size recommendations

### Requirement 6: Execution Preparation

**User Story:** As a trader, I want pre-filled order instructions generated for selected signals, so that I can execute trades quickly and accurately.

#### Acceptance Criteria

1. WHEN a user selects a signal for execution, THE Execution_Preparer SHALL generate buy and sell order instructions within 50ms
2. THE Execution_Preparer SHALL include exchange, symbol, side, order type, price, and quantity in each order
3. THE Execution_Preparer SHALL apply a configurable slippage buffer (default 0.05%) to limit prices
4. THE Execution_Preparer SHALL validate that the order meets exchange minimum notional requirements
5. IF order validation fails, THEN THE Execution_Preparer SHALL return an error with specific validation failure details
6. THE Execution_Preparer SHALL format quantities to exchange-specific precision requirements
7. THE Execution_Preparer SHALL include time-in-force parameters (default: IOC - Immediate or Cancel)
8. THE Execution_Preparer SHALL calculate and display expected fees for each order
9. THE Execution_Preparer SHALL provide order preview with worst-case and expected outcomes
10. THE Execution_Preparer SHALL generate unique client order IDs for tracking
11. THE Execution_Preparer SHALL validate sufficient balance exists (if balance data available)
12. WHEN market conditions change significantly during preparation, THE Execution_Preparer SHALL warn the user

### Requirement 7: Signal and Execution History

**User Story:** As a trader, I want to store and retrieve historical signals and executions, so that I can analyze my trading performance.

#### Acceptance Criteria

1. THE Storage_Service SHALL persist all emitted signals to the database with full detail
2. THE Storage_Service SHALL persist execution instructions and their final outcomes
3. WHEN querying history, THE Storage_Service SHALL support filtering by date range, asset, exchange, and profit range
4. THE Storage_Service SHALL retain data for a configurable retention period (default 90 days)
5. THE Storage_Service SHALL serialize signals and executions to JSON format for persistence
6. THE Storage_Service SHALL deserialize stored JSON back to typed objects for retrieval
7. THE Storage_Service SHALL support pagination for large result sets (default page size 100)
8. THE Storage_Service SHALL index frequently queried fields for performance
9. THE Storage_Service SHALL support data export to CSV format
10. THE Storage_Service SHALL perform automatic cleanup of data beyond retention period
11. THE Storage_Service SHALL maintain referential integrity between signals and executions
12. WHEN storage errors occur, THE Storage_Service SHALL queue writes for retry

### Requirement 8: Simulation Engine

**User Story:** As a trader, I want to simulate arbitrage strategies against historical data, so that I can validate my approach before live trading.

#### Acceptance Criteria

1. THE Simulation_Engine SHALL accept historical order book data as input from file or database
2. THE Simulation_Engine SHALL replay order books in chronological order respecting original timestamps
3. THE Simulation_Engine SHALL compute signals using the same Arbitrage_Engine logic as live mode
4. THE Simulation_Engine SHALL track simulated PnL based on signal outcomes with configurable fill assumptions
5. WHEN simulation completes, THE Simulation_Engine SHALL return aggregate statistics including total PnL, win rate, and max drawdown
6. THE Simulation_Engine SHALL support configurable playback speed (1x to 100x)
7. THE Simulation_Engine SHALL support pause, resume, and step-through modes
8. THE Simulation_Engine SHALL allow parameter sweeps for strategy optimization
9. THE Simulation_Engine SHALL model realistic execution delays and partial fills
10. THE Simulation_Engine SHALL generate detailed trade-by-trade reports
11. THE Simulation_Engine SHALL compare results against buy-and-hold baseline
12. THE Simulation_Engine SHALL support synthetic data generation for stress testing

### Requirement 9: Dashboard Signal Display

**User Story:** As a trader, I want to view live arbitrage signals in a dashboard, so that I can monitor opportunities in real-time.

#### Acceptance Criteria

1. WHEN new signals are computed, THE Dashboard SHALL display them within 500ms of emission
2. THE Dashboard SHALL display signal asset, buy exchange, sell exchange, profit percent, net profit, confidence, and recommended size
3. THE Dashboard SHALL allow sorting signals by any displayed column
4. THE Dashboard SHALL allow filtering signals by asset, exchange, minimum profit, or minimum confidence
5. WHEN a signal is selected, THE Dashboard SHALL show detailed order book visualization for both exchanges
6. THE Dashboard SHALL visually distinguish signals by confidence level (high/medium/low)
7. THE Dashboard SHALL show signal age and auto-expire stale signals from view
8. THE Dashboard SHALL display exchange connection status indicators
9. THE Dashboard SHALL support keyboard shortcuts for common actions
10. THE Dashboard SHALL remember user preferences for sorting and filtering
11. THE Dashboard SHALL provide one-click access to execution preparation
12. THE Dashboard SHALL display system health metrics (latency, throughput)

### Requirement 10: Analytics and Reporting

**User Story:** As a trader, I want analytics charts showing my performance, so that I can improve my trading decisions.

#### Acceptance Criteria

1. THE Analytics_View SHALL display a profit/loss chart over configurable time periods (hour, day, week, month)
2. THE Analytics_View SHALL display confidence score distribution for executed trades as a histogram
3. THE Analytics_View SHALL display per-exchange performance metrics including volume and success rate
4. THE Analytics_View SHALL calculate and display win rate and average profit per trade
5. WHEN data is insufficient for meaningful analysis, THE Analytics_View SHALL display an appropriate message
6. THE Analytics_View SHALL display cumulative PnL curve with drawdown overlay
7. THE Analytics_View SHALL show signal-to-execution conversion rate
8. THE Analytics_View SHALL display average signal lifetime before expiration
9. THE Analytics_View SHALL compare actual vs expected profit for executed trades
10. THE Analytics_View SHALL identify best and worst performing asset pairs
11. THE Analytics_View SHALL show time-of-day performance patterns
12. THE Analytics_View SHALL support date range selection for all charts

### Requirement 11: API Key Security

**User Story:** As a trader, I want my API keys stored securely, so that my exchange accounts are protected.

#### Acceptance Criteria

1. THE Key_Store SHALL encrypt API keys at rest using AES-256-GCM encryption
2. THE Key_Store SHALL never expose API keys to the frontend or logs
3. THE Key_Store SHALL require a master password to decrypt keys on application startup
4. IF an invalid master password is provided, THEN THE Key_Store SHALL deny access and log the attempt without revealing key data
5. THE Key_Store SHALL support key rotation without service interruption
6. THE Key_Store SHALL derive encryption keys using PBKDF2 with minimum 100,000 iterations
7. THE Key_Store SHALL store keys in a platform-appropriate secure location
8. THE Key_Store SHALL support multiple API key sets per exchange (e.g., read-only vs trading)
9. THE Key_Store SHALL validate API key format before storage
10. THE Key_Store SHALL support secure key deletion with memory wiping
11. WHEN the application closes, THE Key_Store SHALL clear decrypted keys from memory
12. THE Key_Store SHALL support optional hardware security module integration

### Requirement 12: Audit Logging

**User Story:** As a trader, I want all actions logged, so that I can review system behavior and troubleshoot issues.

#### Acceptance Criteria

1. THE Audit_Logger SHALL log all signal emissions with timestamps and full signal details
2. THE Audit_Logger SHALL log all execution preparations and user confirmations with order details
3. THE Audit_Logger SHALL log all exchange connection events including connects, disconnects, and errors
4. THE Audit_Logger SHALL log all errors with stack traces and context information
5. THE Audit_Logger SHALL support configurable log levels (DEBUG, INFO, WARN, ERROR)
6. THE Audit_Logger SHALL implement log rotation with configurable size and count limits
7. THE Audit_Logger SHALL write logs to both file and structured storage for querying
8. THE Audit_Logger SHALL include correlation IDs to trace related events
9. THE Audit_Logger SHALL sanitize sensitive data (API keys, passwords) from log output
10. THE Audit_Logger SHALL support real-time log streaming to the UI for debugging
11. THE Audit_Logger SHALL include performance metrics in structured log format
12. THE Audit_Logger SHALL support log export for external analysis tools

### Requirement 13: Configuration Management

**User Story:** As a trader, I want to configure system parameters, so that I can customize behavior to my trading style.

#### Acceptance Criteria

1. THE Config_Manager SHALL load configuration from a TOML file on startup
2. THE Config_Manager SHALL support hot-reload of non-critical configuration without restart
3. THE Config_Manager SHALL validate configuration values against defined schemas
4. IF configuration validation fails, THEN THE Config_Manager SHALL use defaults and log warnings
5. THE Config_Manager SHALL support environment variable overrides for deployment flexibility
6. THE Config_Manager SHALL provide a UI for editing common configuration options
7. THE Config_Manager SHALL maintain configuration version history for rollback
8. THE Config_Manager SHALL export/import configuration for backup and sharing
9. THE Config_Manager SHALL separate sensitive configuration (keys) from general settings
10. THE Config_Manager SHALL provide configuration documentation and tooltips in UI

### Requirement 14: Error Handling and Recovery

**User Story:** As a trader, I want the system to handle errors gracefully, so that I don't miss opportunities due to technical issues.

#### Acceptance Criteria

1. WHEN an exchange connector fails, THE System SHALL continue operating with remaining exchanges
2. WHEN database writes fail, THE System SHALL queue operations for retry with exponential backoff
3. WHEN the UI loses backend connection, THE Dashboard SHALL display a reconnection indicator and auto-reconnect
4. IF critical errors occur, THEN THE System SHALL send desktop notifications to alert the user
5. THE System SHALL implement circuit breakers for failing external services
6. THE System SHALL maintain operation logs for post-incident analysis
7. WHEN recovering from crash, THE System SHALL restore last known good state
8. THE System SHALL validate data integrity on startup and repair if possible
9. THE System SHALL provide manual recovery options in the UI for stuck states
10. THE System SHALL gracefully degrade features when dependencies are unavailable

### Requirement 15: Performance and Scalability

**User Story:** As a trader, I want the system to perform efficiently, so that I can monitor many exchanges without lag.

#### Acceptance Criteria

1. THE System SHALL process order book updates with less than 50ms internal latency
2. THE System SHALL support concurrent connections to at least 10 exchanges
3. THE System SHALL maintain UI responsiveness (60fps) during high-throughput periods
4. THE System SHALL use less than 500MB RAM under normal operation
5. THE System SHALL batch database writes to minimize I/O overhead
6. THE System SHALL use efficient data structures for order book storage (sorted maps)
7. THE System SHALL implement connection pooling for database access
8. THE System SHALL profile and optimize hot paths in signal computation
9. THE System SHALL support configurable resource limits to prevent runaway usage
10. THE System SHALL provide performance metrics dashboard for monitoring
