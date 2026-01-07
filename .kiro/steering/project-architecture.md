---
inclusion: always
---

# Arbitrage Dashboard - Project Architecture

## Project Overview
Professional-grade cryptocurrency arbitrage trading system with institutional-level safety and performance requirements.

## Workspace Structure

```
arbitrage-dashboard/
├── crates/
│   ├── arbitrage-core/          # Core arbitrage logic and strategies
│   ├── arbitrage-server/        # Web server and API endpoints
│   └── exchange-connectors/     # Exchange WebSocket/REST connectors
├── config/                      # Configuration files
├── scripts/                     # Build and development scripts
├── tests/                       # Integration tests
└── Spec/                        # Requirements and design docs
```

## Crate Responsibilities

### arbitrage-core
- **Purpose**: Core arbitrage detection and execution logic
- **Key Components**:
  - Strategy implementations (10 professional strategies)
  - Signal detection and scoring
  - Risk management and position sizing
  - Market data normalization
  - Execution planning

### arbitrage-server
- **Purpose**: Web interface and API server
- **Key Components**:
  - REST API endpoints
  - WebSocket real-time feeds
  - Configuration management
  - Audit logging and security

### exchange-connectors
- **Purpose**: Exchange integration layer
- **Key Components**:
  - WebSocket connection pools
  - REST API clients
  - Data normalization
  - Connection health monitoring

## Critical Design Principles

1. **No Unwrap Rule**: Absolute prohibition on `.unwrap()` usage
2. **Financial Safety**: All calculations use `rust_decimal::Decimal`
3. **Error Propagation**: Comprehensive `Result<T, ArbitrageError>` usage
4. **Performance**: Sub-millisecond latency requirements
5. **Reliability**: 99.9% uptime target with graceful degradation

## Strategy Architecture

Based on the pluggable strategy pattern:
- Each strategy implements the `Strategy` trait
- Unified signal detection and scoring
- Modular execution planning
- Risk-aware position sizing

## Data Flow

```
Exchange WS/REST → Normalizer → Strategy Detection → Signal Scoring → Execution Planning → Order Submission
```

## Key Types

- `Signal`: Arbitrage opportunity representation
- `ExecutionInstruction`: Order execution plan
- `OrderBook`: Normalized market depth data
- `ExchangeId`: Supported exchange enumeration