An AVX2 kernel, and a dispatch rule that will not run a kernel nobody measured.

`arch::kernel` now returns the fastest kernel the silicon has **and** for which
`price::MINTED` holds a row, rather than the fastest one the silicon has. The two used to
be assumed identical and were not: a kernel becomes reachable the moment it compiles, and
its coefficients arrive later, from a human running `examples/mint.rs` on that machine and
pasting rows in. In the gap the crate would price every arming decision on x86_64 with
SSSE3's coefficients and then execute AVX2 — the arithmetic would say a pattern loses and
the hardware would have won, or the reverse, and nothing anywhere would report a
disagreement. Dispatch is now the intersection, so an unpriced kernel is inert rather than
mispriced, and `examples/mint.rs` is the only thing that can wake it.

The kernel itself is 32 bytes per `vpshufb`, which for this crate is two slices per
register rather than one wider slice: the sieve's step is a 16-entry table lookup, and
AVX2's shuffle operates per 128-bit lane, so the natural shape is two independent
trajectories advancing in the two halves. Ungated soundness over the full pattern slate is
unchanged, as it has to be — a wider register is a different schedule for the same
composition and not a different answer.

Because dispatch is now conservative, the newest kernel is the one the differential harness
would reach last. `shuffle::force` is the seam that fixes that: it takes a kernel, refuses
any the runtime probe did not admit, and lets `tests/kernels.rs` and the new `kernels` fuzz
target sweep everything `shuffle::available()` reports instead of only what dispatch chose.
Three prose paragraphs of `SECURITY.md` used to say "both vector paths"; there are three.

Three measured byte priors join `SOURCE` — `PROSE`, `JSON`, and `LOG` — minted from pinned
public corpora (NLTK's Gutenberg selection, `simdjson-data`, and `loghub`) and re-derived
on every push by `.github/workflows/priors.yml`, which fails on a drifted cell. A prior is
a claim about what bytes a document is made of, and one corpus of Rust was answering that
question for JSON and for logs. Adding chains can only tighten the fallthrough the gate
estimates, never loosen it, so this is a coverage change and not a soundness one.

Minting a row for a thin corpus used to produce an invalid distribution rather than a
refusal: `LOG`'s `High` row was all zeros, because no pair in the corpus reached it. A
support floor now makes any row under 1024 observed pairs absorbing — self-looping with
probability 1.0 — which is both a valid distribution and the pessimistic reading of a state
nothing was measured about.
