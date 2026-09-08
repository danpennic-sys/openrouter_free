# Tests

## Unit tests (always run)

```bash
cargo test
```

These cover serialization, defaults, and client construction. No network required.

## Integration / network tests

Network tests live in `tests/integration.rs` and are gated on `OPENROUTER_API_KEY`.

```bash
export OPENROUTER_API_KEY=sk-or-...
cargo test --test integration -- --nocapture
```

To run only the offline unit tests inside the integration binary:

```bash
cargo test --test integration -- --skip network
```

## CI behavior

The GitHub Actions workflow runs `cargo test --all-features`.  
Network tests automatically skip when the secret is absent, so CI stays green without credentials.
