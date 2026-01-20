# Coding Standards

## Rust Guidelines

### Style

- Run `cargo fmt` before committing (rustfmt.toml config in repo)
- Use 4 spaces for indentation
- Max line width: 100 characters
- No trailing whitespace

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Modules | snake_case | `exchange_connectors` |
| Structs | UpperCamelCase | `OrderBook` |
| Enums | UpperCamelCase | `ExchangeId` |
| Trait | UpperCamelCase | `ExchangeConnector` |
| Functions | snake_case | `calculate_profit` |
| Constants | SCREAMING_SNAKE_CASE | `MAX_ORDER_SIZE` |
| Type parameters | UpperCamelCase | `T: Clone` |
| Variables | snake_case | `order_book` |

### Error Handling

- Use `Result<T, E>` for recoverable errors
- Use `panic!()` only for unrecoverable bugs
- Define errors in `error.rs` with `thiserror`
- Never log errors with `unwrap()` or `expect()`
- Propagate errors with `?` operator

**Good:**
```rust
fn parse_order_book(data: &Value) -> Result<OrderBook> {
    let bids = data["bids"].as_array()
        .ok_or_else(|| ArbitrageError::Parse("missing bids".into()))?;
    Ok(OrderBook { bids: parse_bids(bids)? })
}
```

**Bad:**
```rust
fn parse_order_book(data: &Value) -> OrderBook {
    let bids = data["bids"].as_array().unwrap(); // panic on bad data
    OrderBook { bids: parse_bids(bids).unwrap() }
}
```

### Async/Await

- Use `async` for all I/O operations
- Never block in async code (use `tokio::time::timeout` instead of `std::thread::sleep`)
- Prefer `tokio::sync` primitives over std equivalents
- Use `#[tokio::test]` for async tests

### Testing

- Every public function should have at least one test
- Unit tests in same file: `#[cfg(test)] mod tests { ... }`
- Integration tests in `tests/` directory
- Property tests with proptest for numerical functions
- Use `test_utils` fixtures instead of duplicating setup code

### Performance

- Profile before optimizing (use `cargo bench` and flamegraph)
- Prefer `DashMap` over `RwLock<HashMap>` for hot paths
- Use `Arc<RwLock<T>>` when single-threaded access dominates
- Clone is cheaper than locking - consider `Arc::clone()`
- Batch operations when possible (subscribe to 10 symbols at once)

### Documentation

- Document all public items with `///` doc comments
- Include examples in docs: `/// # Example`
- Document error variants: `/// Errors when ...`
- Document performance expectations: `/// O(n) in symbol count`
- Don't document trivial getters

**Good:**
```rust
/// Calculates profit in basis points between buy and sell prices.
///
/// # Errors
/// Returns `ZeroDivisionError` if prices are zero.
pub fn calculate_profit_bps(buy: Decimal, sell: Decimal) -> Result<i32, Error> {
```

**Bad:**
```rust
/// Calculate profit
pub fn calculate_profit_bps(buy: Decimal, sell: Decimal) -> i32 {
```

## Git Conventions

### Commit Messages

```
<type>(<scope>): <subject>

<body>

Footer
```

Types:
- `feat` - New feature
- `fix` - Bug fix
- `refactor` - Code restructuring
- `chore` - Build/tooling changes
- `docs` - Documentation only
- `test` - Adding/modifying tests

### Branch Names

- `feature/<name>` - New features
- `fix/<issue-number>` - Bug fixes
- `refactor/<scope>` - Restructuring
- `issue<number>-<desc>` - Issue worktrees

### Pull Requests

- Keep PRs focused (< 500 lines changed ideal)
- Include description of changes
- Link related issues
- Request review from maintainers
