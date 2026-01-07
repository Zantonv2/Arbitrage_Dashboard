---
inclusion: always
---

# Implementation Roadmap - Arbitrage Dashboard

## Current Status Summary

### ✅ COMPLETED (Ready for Use)
- **Core Types System**: Complete with all financial data structures
- **Error Handling**: Comprehensive ArbitrageError with proper propagation
- **Basic Arbitrage Engine**: 85% complete with signal detection and deduplication
- **Normalizer**: 80% complete with symbol mapping and validation
- **Exchange Connectors**: Partial implementations for major exchanges

### ❌ CRITICAL BLOCKERS (Must Implement for MVP)
1. **All 10 Strategy Implementations** - Currently only stubs exist
2. **Size Calculator Module** - No trade size optimization
3. **Execution Preparer Module** - No order generation capability
4. **Storage Module** - No persistence layer
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

### Phase 4: Testing & Validation (Priority: HIGH)
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
1. **CEX ↔ CEX Price Arbitrage** - Most common, easiest to implement
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

1. **Start with Size Calculator** - Blocking execution preparation
2. **Implement CEX Arbitrage Strategy** - Simplest and most common
3. **Add Property Tests** - Critical for financial correctness
4. **Complete Storage Module** - Needed for persistence and analytics
5. **Wire API Endpoints** - Enable frontend integration