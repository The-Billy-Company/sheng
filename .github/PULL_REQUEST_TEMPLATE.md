## What & why

<!-- The change and the reason for it. Link an issue if one exists. -->

## Evidence

<!-- Sheng's soundness and cost claims are only as good as what backs them.
Fill in whatever applies, delete what doesn't: -->

- [ ] `cargo test --release` passes locally (both kernel paths if you can —
      see `src/arch/README.md` for how the scalar/NEON/SSSE3 kernels are
      selected and cross-checked).
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean.
- [ ] Touched an `unsafe` block: every block still carries a `// SAFETY:`
      comment that argues the actual precondition, not just states one.
- [ ] Changed selectivity/pricing constants: re-minted via `examples/mint.rs`
      on real hardware, not hand-edited (see `src/price/README.md`).
- [ ] Changed a byte prior: re-minted from the corpus its doc comment names, not
      hand-edited — `priors.yml` re-derives the pinned three and fails on a
      drifted cell (see `src/prior/minted.rs`).
- [ ] Added or changed a soundness-relevant path: ran the fuzz target that
      covers it locally (`cd fuzz && ./seeds.sh && cargo +nightly fuzz run
      {soundness,skip,kernels} -- -max_total_time=90`); see `fuzz/README.md` for
      which target holds which property.
- [ ] Added a `towncrier` changelog fragment under `changelog.d/` if this is
      user-visible.

## Compatibility

<!-- Does this change the public API (`Sieve`, `Policy`, re-exports from
`lib.rs`)? Sheng follows semver — call out any breaking change explicitly. -->
