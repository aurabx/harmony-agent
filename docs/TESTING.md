# Testing Guide for wg-agent

This document describes the testing strategy and how to run tests for the wg-agent project.

## Test Coverage Target

Our target is **> 80% code coverage** across all modules.

## Test Categories

### Unit Tests

Unit tests are located alongside the source code in `src/` directories.

```bash
# Run all unit tests
cargo test --lib

# Run tests for a specific module
cargo test --lib config::
cargo test --lib wireguard::
cargo test --lib security::
```

### Integration Tests

Integration tests are in the `tests/` directory and test interactions between modules.

```bash
# Run integration tests
cargo test --test integration_test

# Run all integration tests
cargo test --test '*'
```

### Doc Tests

Documentation tests ensure examples in documentation work correctly.

```bash
# Run doc tests
cargo test --doc
```

### Benchmarks

Performance benchmarks using Criterion.

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench -- key_generation
cargo bench -- monitoring
```

## Running Tests

### Quick Test Run

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run with specific number of threads
cargo test -- --test-threads=4
```

### Comprehensive Test Run

```bash
# Run all tests with coverage
cargo test --all-features --workspace

# Check formatting
cargo fmt -- --check

# Run linter
cargo clippy --all-targets --all-features -- -D warnings
```

## Code Coverage

### Using cargo-llvm-cov

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --all-features --workspace

# Generate HTML report
cargo llvm-cov --all-features --workspace --html

# Generate LCOV format (for CI)
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
```

### Using tarpaulin (Linux only)

```bash
# Install cargo-tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --out Html --output-dir coverage
```

## Platform-Specific Testing

### Linux

```bash
# Run with Linux-specific features
cargo test --features linux

# Test with elevated privileges (required for some tests)
sudo -E cargo test
```

### macOS

```bash
# Run with macOS-specific features
cargo test --target aarch64-apple-darwin
cargo test --target x86_64-apple-darwin
```

### Windows

```bash
# Run with Windows-specific features
cargo test --target x86_64-pc-windows-gnu
```

## Docker-Based Testing

### Build Test Image

```bash
# Build Docker image
docker build -t wg-agent-test .

# Run tests in Docker
docker run --rm wg-agent-test cargo test
```

### Docker Compose Test Environment

Create a `docker-compose.test.yml`:

```yaml
version: '3.8'
services:
  test:
    build: .
    command: cargo test --workspace
    volumes:
      - .:/usr/src/wg-agent
    environment:
      - RUST_BACKTRACE=1
```

Run tests:

```bash
docker-compose -f docker-compose.test.yml up --abort-on-container-exit
```

## Continuous Integration

Our CI/CD pipeline runs on GitHub Actions:

- **Test Suite**: Runs on Linux, macOS, and Windows with stable and beta Rust
- **Code Coverage**: Generates coverage reports and uploads to codecov.io
- **Security Audit**: Checks for known vulnerabilities
- **Benchmarks**: Tracks performance over time
- **Docker Build**: Ensures Docker image builds successfully

See `.github/workflows/ci.yml` for details.

## Writing Tests

### Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = setup_test_data();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected_value);
    }
}
```

### Integration Test Template

```rust
use wg_agent::module::{Component, Config};

#[test]
fn test_integration_scenario() {
    // Setup
    let config = Config::default();
    let component = Component::new(config);
    
    // Execute
    let result = component.perform_action();
    
    // Verify
    assert!(result.is_ok());
}
```

### Benchmark Template

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_my_function(c: &mut Criterion) {
    c.bench_function("my_function", |b| {
        b.iter(|| {
            my_function(black_box(input))
        });
    });
}

criterion_group!(benches, bench_my_function);
criterion_main!(benches);
```

## Test Helpers and Fixtures

Common test utilities are available in `tests/common/mod.rs`:

```rust
// Create test configuration
let config = create_test_config();

// Create temporary directory
let temp_dir = create_temp_dir();

// Generate test keys
let (private_key, public_key) = generate_test_keypair();
```

## Best Practices

1. **Test Independence**: Tests should not depend on each other
2. **Cleanup**: Always clean up resources (files, network connections)
3. **Deterministic**: Tests should produce consistent results
4. **Fast**: Keep tests fast to encourage frequent running
5. **Clear Names**: Use descriptive test names that explain what's being tested
6. **Arrange-Act-Assert**: Follow the AAA pattern for clarity
7. **Edge Cases**: Test boundary conditions and error cases
8. **Documentation**: Add comments for complex test scenarios

## Troubleshooting

### Test Failures

```bash
# Run a specific test with output
cargo test test_name -- --nocapture

# Show backtrace on panic
RUST_BACKTRACE=1 cargo test

# Run ignored tests
cargo test -- --ignored

# Run serially (helps with race conditions)
cargo test -- --test-threads=1
```

### Permission Issues

Some tests require elevated privileges:

```bash
# Linux/macOS
sudo -E cargo test

# Or run specific tests that don't require privileges
cargo test --lib
```

### Slow Tests

```bash
# Skip slow tests
cargo test -- --skip slow_test

# Run only fast tests
cargo test --lib
```

## Test Metrics

Current test statistics:
- **Total Tests**: 94+ (unit + integration)
- **Coverage**: Target > 80%
- **Modules Tested**: All core modules
- **Platform Coverage**: Linux, macOS, Windows (CI)

## Contributing

When adding new features:

1. Write tests first (TDD)
2. Ensure all tests pass
3. Add integration tests for cross-module functionality
4. Update benchmarks if performance-critical
5. Document any new test helpers

## Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Criterion.rs Guide](https://bheisler.github.io/criterion.rs/book/)
- [proptest Documentation](https://altsysrq.github.io/proptest-book/)
- [GitHub Actions for Rust](https://github.com/actions-rs)
