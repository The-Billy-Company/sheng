---
name: Bug report
about: Sheng refuted, refused, or crashed on input it shouldn't have
title: ""
labels: bug
assignees: ""
---

## What happened

<!-- A clear description of the incorrect behavior. -->

## Minimal reproduction

```rust
// The pattern, the `Sieve` construction (gated or ungated, `Policy` used),
// and the haystack that triggers it. Smaller is better — a fuzz-minimized
// input is ideal if you have `cargo fuzz tmin` available.
```

## Expected vs. actual

- **Expected:** the sieve should not refute this haystack / should build without panicking / ...
- **Actual:** ...

## Environment

- `sheng` version:
- `rustc --version`:
- Target architecture (`aarch64`/NEON, `x86_64`/SSSE3, or scalar fallback):
- OS:

## Additional context

<!-- Anything else relevant — did this regress from a prior version, does it
only reproduce under Miri, only in release mode, only with a specific
`regex-automata` version, etc. -->
