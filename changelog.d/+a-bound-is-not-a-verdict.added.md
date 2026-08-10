A counted repeat no longer refuses a pattern before anything has been priced. `Policy::relax`
is on by default and relaxes bounded repetitions to unbounded ones before the projection
runs, which is sound in the one direction this crate cares about: dropping a bound yields a
**superset** language, and a sieve over a superset still cannot reject a document the
pattern matches. `src/dfa.rs` already granted that permission for a caller supplying their
own automaton; `src/relax.rs` is the crate taking it for a pattern string.

The refusal it removes was the worst kind the crate had — a *ceiling* rather than a verdict,
reached without reading a single measured coefficient, for a reason with nothing to do with
whether a filter would pay. A bounded repeat spends a DFA state per count, so
`AKIA[0-9A-Z]{16}` alone put the reachable core past `MAX_CORE_STATES` and came back
`Decline::TooWide`. That shape — literal prefix, then a counted run of a distinctive
alphabet — is essentially every credential in circulation, which made it a whole product
surface refused unpriced. `examples/census.rs` now prints the pair as a standing
measurement: **14 of 31 patterns refused structurally with relaxation off, 1 with it on.**

Relaxation is a seam rather than a silent improvement, because it can also *cost*
selectivity — a relaxed quotient is coarser and may retire fewer documents. So both
candidates are built and priced against each other and the better one is kept, which is
what `tests/relax.rs` holds: the chosen candidate is never priced worse than the strict one,
`policy.relax = false` reproduces the strict build exactly, and a relaxed sieve never
refutes a document that matches. The strict automaton still prices the rival and still
confirms every survivor, so nothing downstream of the build learns that a bound was ever
dropped.

`MAX_CORE_STATES` is now public, since it is the number a caller reads to understand a
`TooWide` decline and it was previously nameable only in prose. `regex-syntax` becomes a
direct dependency and adds nothing to the graph — `regex-automata`'s own `syntax` feature
already resolves exactly that crate, and it is named here only to keep the two on one copy
of `Hir`. It stays out of the public API, so its next major version is not a breaking
change here.
