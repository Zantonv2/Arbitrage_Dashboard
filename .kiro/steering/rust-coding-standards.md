---
inclusion: always
---

# Rust Coding Standards - CRITICAL RULES

## 🚨 ABSOLUTE PROHIBITIONS

### NO UNWRAP PATTERN - EVER
**VIOLATION OF THIS RULE IS CONSIDERED A CRITICAL ERROR**

- **NEVER** use `.unwrap()` in any code
- **NEVER** use `.expect()` without proper error context
- **NEVER** use `panic!()` in production code
- **NEVER** use `unreachable!()` without exhaustive proof

### REQUIRED ERROR HANDLING PATTERNS

```rust
// ✅ CORRECT - Proper error handling
match result {
    Ok(value) => value,
    Err(e) => return Err(ArbitrageError::from(e)),
}

// ✅ CORRECT - Using ? operator
let value = operation().map_err(ArbitrageError::from)?;

// ✅ CORRECT - Safe unwrapping with fallback
let value = option.unwrap_or_default();
let value = option.unwrap_or_else(|| calculate_fallback());

// ❌ FORBIDDEN - Direct unwrap
let value = result.unwrap(); // NEVER DO THIS

// ❌ FORBIDDEN - Lazy expect
let value = result.expect("failed"); // NEVER DO THIS
```

## Error Handling Requirements

1. **All functions that can fail MUST return `Result<T, ArbitrageError>`**
2. **Use proper error propagation with `?` operator**
3. **Provide meaningful error context with `.map_err()`**
4. **Handle all edge cases explicitly**

## Financial Safety Requirements

1. **All decimal calculations MUST use `rust_decimal::Decimal`**
2. **Never use `f64` for financial calculations**
3. **Always validate input ranges before calculations**
4. **Implement overflow protection for all arithmetic**

## Performance Requirements

1. **Minimize allocations in hot paths**
2. **Use `&str` instead of `String` where possible**
3. **Prefer iterators over collecting to vectors**
4. **Use `Arc<T>` for shared immutable data**

## Testing Requirements

1. **NO TESTS IN SOURCE FILES - EVER**
2. **All tests MUST be in dedicated test files only**
3. **Every public function MUST have unit tests in separate test files**
4. **Property-based tests for financial calculations in tests/ directory**
5. **Integration tests for external API interactions in tests/ directory**
6. **Mock all external dependencies in tests**