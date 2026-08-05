`Decline::MatchesEmpty` was checking `dfa.is_match_state(start)` directly, but
`regex-automata`'s start state is never itself a match state - not even for a
pattern that matches the empty string - because it encodes "about to try",
not "already matched"; the empty-match acceptance only shows up one step
later, at the end-of-input transition (exactly the `Row.accepts` computation
a few lines below it already uses). The guard could not fire for any pattern,
on any input, ever - a property test sweeping thousands of generated patterns
through `Sieve::new` turned this up directly, by fuzzing the one input the
existing suites always assumed was already well-formed: the pattern itself.

In practice every pattern that matches empty still declined - via the lattice
harvest finding no discriminating quotient and returning `NoQuotient` instead
- so this was never a soundness gap, only the wrong reason and a wasted
harvest. `Projection::of` now checks `next_eoi_state(start)` too, and
`a*`, `.*`, the empty pattern, and `a**` all now decline with the specific,
documented `MatchesEmpty` reason instead of the generic `NoQuotient`.

Added `tests/errors.rs`: a property suite over the build path specifically,
sweeping thousands of generated and garbage pattern strings through
`Sieve::new` to prove it never panics, plus directed coverage proving every
`BuildError`/`Decline` variant is actually reachable, explains itself with the
crate's shared `"no sieve: …"` prefix, and reports no `source()` it doesn't
have.
