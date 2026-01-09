---
inclusion: always
---

# Implementation Roadmap - Arbitrage Dashboard

## Current Status Summary (Updated: January 2026)

### ✅ COMPLETED (Production Ready)
- **Core Types System**: Complete with all financial data structures
- **Error Handling**: Comprehensive ArbitrageError with proper propagation
- **Basic Arbitrage Engine**: 85% complete with signal detection and deduplication
- **Normalizer**: 80% complete with symbol mapping and validation
- **Exchange Connectors**: ✅ **PRODUCTION READY** - All 6 connectors working (~95% test pass rate)
  - OKX (18/19 tests)
  - ByBit (18/19 tests)
  - MEXC (17/19 tests)
  - Gate.io (17/18 tests)
  - Kraken (17/18 tests)
  - Bitstamp (17/18 tests)
- **CEX Arbitrage Strategy**: Base implementation complete with detect/filter/plan

### ❌ CRITICAL BLOCKERS (Must Implement for MVP)
1. **Remaining 9 Strategy Implementations** - CEX Arbitrage done, 9 more needed
2. **Size Calculator Module** - Stub exists, needs full implementation
3. **Execution Preparer Module** - Stub exists, needs full implementation
4. **Storage Module** - Stub exists, needs SQLite persistence
5. **Property Tests** - Only 6 of 45 required tests implemented

### ⚠️ PARTIAL IMPLEMENTATIONS (Need Completion)
- **Confidence Scorer**: Missing advanced algorithms (Bayesian, regime detection)
- **Server Routes**: API endpoints exist but need backend wiring
- **WebSocket Broadcasting**: Needs completion for real-time feeds
- **Simulation Engine**: Declared but not implemented

## Implementation Phases

### Phase 1: Core Engine Completion (Priority: CRITICAL)
**Estimated Time**: 1-2 weeks

**Tasks**:
1. Implement Size Calculator module with order book depth analysis
2. Implement Execution Preparer with order instruction generation
3. Complete Storage module with SQLite persistence
4. Add comprehensive property tests for all core modules
5. Complete confidence scorer advanced algorithms

**Success Criteria**:
- All core modules have working implementations
- Property tests pass for financial calculations
- Storage can persist and query signals/executions

### Phase 2: Strategy System Implementation (Priority: CRITICAL)
**Estimated Time**: 2-3 weeks

**Tasks**:
1. Define base Strategy trait with detect/filter/plan methods
2. Implement all 10 strategy modules:
   - CEX ↔ CEX Price Arbitrage
   - Spot ↔ Perpetual Arbitrage
   - Funding Rate Arbitrage
   - Hedged Funding Strategy
   - Cross-Exchange Arbitrage
   - Spread Capture Strategy
   - Latency Arbitrage
   - Stablecoin Peg Arbitrage
   - Convergence Arbitrage
   - New Listing Arbitrage
3. Integrate strategies with arbitrage engine
4. Add strategy-specific tests and validation

**Success Criteria**:
- All 10 strategies detect opportunities correctly
- Strategy scoring and filtering works
- Integration tests pass for strategy pipeline

### Phase 3: Backend Integration (Priority: HIGH)
**Estimated Time**: 1-2 weeks

**Tasks**:
1. Complete Simulation engine for backtesting
2. Wire up all API endpoints with backend logic
3. Implement WebSocket real-time signal broadcasting
4. Complete configuration management system
5. Add audit logging integration

**Success Criteria**:
- API endpoints return real data
- WebSocket feeds work in real-time
- Configuration hot-reload functions
- All operations are logged for audit

### Phase 4: Order Execution System (Priority: HIGH)
**Estimated Time**: 2-3 weeks

**Tasks**:
1. Add trading methods to exchange connectors (`place_order`, `cancel_order`, `get_order_status`)
2. Implement OrderExecutor service for coordinated execution
3. Add secure API key management in KeyStore
4. Implement execution tracking and order lifecycle management
5. Add rollback/hedge logic for failed executions

**Success Criteria**:
- Can place and track orders on all 6 exchanges
- Simultaneous arbitrage execution works reliably
- Order failures are handled gracefully with rollback
- All executions are logged for audit

### Phase 5: Frontend Dashboard (Priority: MEDIUM)
**Estimated Time**: 2-3 weeks

**Technology Stack**:
- **Frontend**: SvelteKit + Vite + Tailwind CSS
- **Real-time**: WebSocket connection to backend
- **Charts**: Chart.js or D3.js for signal visualization
- **State**: Svelte stores for real-time data management

**Tasks**:
1. Create SvelteKit project structure with Tailwind
2. Implement real-time signal dashboard with WebSocket
3. Add order book visualization and exchange status
4. Create execution management interface (prepare/confirm trades)
5. Add analytics dashboard (P&L, strategy performance)
6. Implement configuration management UI

**Success Criteria**:
- Real-time signal updates via WebSocket
- Interactive order book and price charts
- Manual execution interface for paper trading
- Analytics dashboard showing strategy performance
- Responsive design for desktop and mobile

**Frontend Architecture**:
- **Control Panel Only**: Frontend is a control panel for monitoring and manual execution
- **Heavy Lifting in Rust**: All arbitrage detection, execution, and data processing in backend
- **Real-time Updates**: WebSocket feeds for live signals, order books, and execution status
- **Manual Execution**: Users can review and manually confirm arbitrage opportunities
- **Analytics Dashboard**: Performance metrics, P&L tracking, strategy effectiveness
- **Configuration UI**: Manage exchange connections, strategy parameters, risk limits

### Phase 6: Testing & Validation (Priority: HIGH)
**Estimated Time**: 1 week

**Tasks**:
1. Implement all 45 property tests from design document
2. Run comprehensive integration tests
3. Performance profiling and optimization
4. Security audit of key management
5. Load testing for concurrent operations

**Success Criteria**:
- All property tests pass
- System handles expected load (100+ signals/second)
- Security vulnerabilities addressed
- Performance meets sub-100ms latency requirement

## Strategy Implementation Priority

### Tier 1 (Implement First - Highest ROI)
1. **CEX ↔ CEX Price Arbitrage** - Base implementation complete
2. **Funding Rate Arbitrage** - Passive income, lower risk
3. **Stablecoin Peg Arbitrage** - Frequent opportunities, predictable

### Tier 2 (Implement Second - Medium Complexity)
4. **Spot ↔ Perpetual Arbitrage** - Requires futures data
5. **Cross-Exchange Arbitrage** - Needs balance management
6. **New Listing Arbitrage** - Event-driven opportunities

### Tier 3 (Implement Last - Advanced Features)
7. **Latency Arbitrage** - Requires ultra-low latency
8. **Spread Capture** - Passive market making
9. **Convergence Arbitrage** - Statistical modeling required
10. **Hedged Funding** - Complex position management

## Critical Dependencies

### External Dependencies
- **Exchange WebSocket Feeds**: Must be stable and low-latency
- **Market Data Quality**: Order books must be accurate and fresh
- **Network Latency**: Sub-100ms requirement for execution

### Internal Dependencies
- **Error Handling**: All modules must use Result<T, ArbitrageError>
- **Financial Safety**: All calculations must use Decimal (never f64)
- **Concurrency**: Must handle concurrent signal processing
- **Testing**: Property tests must validate all financial logic

## Risk Mitigation

### Technical Risks
- **Latency Issues**: Implement connection pooling and local caching
- **Data Quality**: Add comprehensive validation and anomaly detection
- **Concurrency Bugs**: Use proven concurrent data structures (DashMap)
- **Financial Errors**: Extensive property-based testing for calculations

### Business Risks
- **Market Volatility**: Implement dynamic confidence scoring
- **Exchange Downtime**: Graceful degradation and failover
- **Regulatory Changes**: Modular design for easy compliance updates
- **Competition**: Focus on execution speed and accuracy

## Success Metrics

### Technical Metrics
- **Latency**: < 100ms from signal detection to execution preparation
- **Accuracy**: > 99.9% correct profit calculations
- **Uptime**: > 99.9% system availability
- **Throughput**: Handle 100+ signals per second

### Business Metrics
- **Signal Quality**: > 80% of signals result in profitable trades
- **Risk Management**: < 1% of trades exceed risk limits
- **Performance**: Achieve projected 8-22% daily returns
- **Reliability**: < 0.1% execution failures

## Next Immediate Actions

1. **Implement Remaining 9 Strategies** - CEX Arbitrage done, continue with Tier 1 strategies
2. **Complete Size Calculator** - Blocking execution preparation
3. **Complete Execution Preparer** - Needed for order generation
4. **Add Property Tests** - Critical for financial correctness
5. **Complete Storage Module** - Needed for persistence and analytics

## Exchange Connector Status

All 6 exchange connectors are **production ready** with the following capabilities:
- ✅ `fetch_symbols` - Get available trading pairs
- ✅ `fetch_order_book` - Get order book data (REST)
- ✅ `fetch_tickers` - Get price/volume data
- ✅ `fetch_funding_rates` - Get funding rates (where supported)
- ✅ `connect` / `disconnect` - WebSocket lifecycle management
- ✅ `subscribe_order_books` - Real-time order book subscriptions
- ✅ `health_check` - Connection health monitoring

### Supported Exchanges
| Exchange | REST API | WebSocket | Funding Rates | Test Pass Rate |
|----------|----------|-----------|---------------|----------------|
| OKX      | ✅       | ✅        | ✅            | 94.7%          |
| ByBit    | ✅       | ✅        | ✅            | 94.7%          |
| MEXC     | ✅       | ✅        | ✅            | 89.5%          |
| Gate.io  | ✅       | ✅        | ❌            | 94.4%          |
| Kraken   | ✅       | ✅        | ❌            | 94.4%          |
| Bitstamp | ✅       | ✅        | ❌            | 94.4%          |