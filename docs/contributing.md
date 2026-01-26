# Contributing

## Getting Started

### Prerequisites

- Rust 1.85+
- Git
- API keys for exchanges you want to test

### Setting Up

1. Fork the repository on GitHub
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR-USER/Arbitrage_Dashboard.git
   cd Arbitrage_Dashboard
   ```
3. Add upstream remote:
   ```bash
   git remote add upstream https://github.com/Zantonv2/Arbitrage_Dashboard.git
   ```
4. Create a feature branch:
   ```bash
   git checkout -b feature/your-feature
   ```

### Development Workflow

1. Make changes following coding standards
2. Run tests:
   ```bash
   cargo test --workspace
   cargo clippy --workspace
   cargo fmt --check
   ```
3. Commit with conventional message
4. Push to your fork
5. Open a Pull Request

### What to Contribute

**High Priority:**
- Bug fixes with tests
- Performance improvements with benchmarks
- Missing exchange connectors
- Missing strategy implementations

**Medium Priority:**
- Documentation improvements
- Test coverage expansion
- Error message improvements
- Configuration options

**Low Priority:**
- Refactoring for readability
- Code style improvements
- Deprecation warnings cleanup

### Adding an Exchange Connector

1. Create file: `crates/exchange-connectors/src/connections/new_exchange.rs`
2. Implement `ExchangeConnector` trait
3. Add to `ALL_EXCHANGES` const in `lib.rs`
4. Add tests in `tests/connector_unit_tests.rs`
5. Update README exchange list

Example structure:
```rust
#[derive(Debug, Clone)]
pub struct NewExchangeConnector {
    base: ConnectorBase,
    rest_client: RestClient,
    ws_manager: WebSocketManager,
}

#[async_trait]
impl ExchangeConnector for NewExchangeConnector {
    fn exchange_id(&self) -> ExchangeId { ExchangeId::NewExchange }
    // ... implement required methods
}
```

### Adding a Strategy

1. Create file: `crates/arbitrage-core/src/strategies/strategy_impl/new_strategy.rs`
2. Implement `Strategy` trait
3. Add to `STRATEGY_REGISTRY` in `mod.rs`
4. Add unit tests
5. Add integration test in `tests/`

### Testing Guidelines

- Unit tests: Test single function behavior
- Integration tests: Test multi-component interaction
- Property tests: Test numerical invariants with proptest
- Integration tests against mocks: Use `MockConnector`

Run specific test types:
```bash
cargo test --lib                    # Unit tests
cargo test --test '*arbitrage*'     # Strategy tests
cargo test -p exchange-connectors   # Connector tests
cargo bench                         # Performance tests
```

### Code Review Expectations

Reviewers will check:
- Tests pass and coverage maintained
- Clippy warnings addressed
- Code follows coding standards
- Documentation updated for public APIs
- Commit messages follow conventions

### Issue Tracking

- Check existing issues before creating new ones
- Tag issues appropriately (bug, feature, enhancement)
- Link PRs to issues they resolve
- Close resolved issues after merge

### Getting Help

- Search existing issues and PRs
- Ask in issue comments for clarification
- Tag questions with `question` label

## Code Maintenance

### Dead Code Management

To prevent accumulation of dead code and maintain code quality:

#### Quarterly Dead Code Audit
Every quarter, perform a comprehensive dead code audit:

1. **Audit Process:**
   ```bash
   # Find all dead code instances
   grep -r "#\[allow(dead_code)\]" --include="*.rs" | wc -l
   grep -r "#\[allow(dead_code)\]" --include="*.rs"
   ```

2. **Classification:**
   - **Remove**: Truly unused functions, fields, imports
   - **Document**: Intentionally kept code with future use plans
   - **Refactor**: Dead code that should be integrated

3. **Acceptance Criteria:**
   - Reduce total `#[allow(dead_code)]` instances by 30+ per year
   - Remove or use at least 10 unused functions per audit
   - Document any remaining dead code with clear rationale
   - Update contribution guidelines with audit process

4. **Tracking:**
   - Create GitHub issue for quarterly audit tracking
   - Document findings in technical debt documentation
   - Update development metrics

#### Pre-commit Prevention
Configure pre-commit hooks to catch dead code:
```bash
# Add to .pre-commit-config.yaml
- repo: local
  hooks:
    - id: check-dead-code
      name: Check for dead code
      entry: bash -c 'grep -r "#\[allow(dead_code)\]" --include="*.rs" . || true'
      language: system
```

#### Code Review Guidelines
When reviewing PRs, check for:
- New `#[allow(dead_code)]` additions require justification
- Unused imports and variables should be removed
- Experimental code should be feature-gated
- Documentation for intentionally kept dead code

### Technical Debt Tracking

Track dead code reduction as part of technical debt:
- Use labels: `tech-debt`, `dead-code`, `cleanup`
- Prioritize dead code removal in quarterly planning
- Measure reduction in code metrics
- Update architectural documentation when removing unused code
