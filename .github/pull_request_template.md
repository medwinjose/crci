## Summary
<!-- What does this PR do? -->

## Checklist
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo fmt --all -- --check` is clean
- [ ] `docs/` updated if behaviour or a documented number changed
- [ ] `docs/tla/CRCI.tla` and its exports updated if the strike rule, window, seen cache or anti-entropy changed
- [ ] New public functions have doc comments
