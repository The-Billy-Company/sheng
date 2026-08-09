`price::Residency` — a required third fact on every `Policy`, naming where the bytes
about to be searched are coming from. `Policy::new(residency)` replaces `Policy::default`,
and `Sieve::new` and `Sieve::of_dfa` each take one.

A per-byte price is only a price against a particular memory system, and this crate was
shipping one column of a two-column measurement. Timed over the same tree at 1 MiB and
64 MiB, each with its own byte marginals:

| | cache-resident | memory-resident | |
|---|---|---|---|
| `dfa_skip` | 0.0124 | 0.0175 | a `memchr` is **41% cheaper** on resident bytes |
| `dfa_excursion` | 8.06 | 9.75 | a dense-DFA re-entry, **21% cheaper** |

`dfa_walk` and `sieve` carry no regime index, and that is a claim rather than an
omission: a dependent-load walk waits on L1 for a table it has already pulled in, and the
composition kernel is issue-bound at three operations a byte. Neither has headroom a
hotter haystack could give it. The two that do carry one are exactly the two that reach
memory.

That asymmetry is the whole mechanism. `rival_per_byte` caps the engine at `dfa_walk`, so
a pattern whose escape set is *frequent* is pinned at a regime-free cap and its verdict
holds everywhere; a pattern whose escape set is *rare* rides `dfa_skip` and is exposed to
the regime completely. `panic!\(` is the second kind, and it is why this exists: it prices
at 1.09x cache-resident and declines, and 1.62x memory-resident and arms — one pattern,
one machine, two correct answers. Before this it took the memory-resident answer in both
regimes, armed on a 0.5 MiB corpus, and measured **0.566x**.

The uncomfortable half, stated where a caller will read it: a sieve's edge over an
accelerated engine comes substantially from *that engine missing cache*. Remove the
memory pressure and the edge shrinks rather than merely rescaling. This is the first
thing in the cost model that scale invariance does not cover — a clock or a thermal state
multiplies every coefficient together and moves no decision, while moving a haystack into
cache rescales two coefficients and leaves two alone.

There is deliberately no `Default`. Both choices are wrong in a way that matters:
defaulting to memory-resident arms patterns that lose on a cached corpus, and defaulting
to cache-resident silently withholds real speedups from the callers this crate is best
at. A regime a row has not measured resolves to `UNMEASURED` and declines, one column in
from the refusal the crate already makes about a whole machine — so `x86_64` callers
declaring `Residency::Cache` get `Uncalibrated` until `.github/workflows/mint.yml` fills
that column on real hardware, rather than memory-resident numbers that are 41% too
generous about the engine.

`examples/survey.rs` reads the regime off the corpus instead of being told, since it
knows how many bytes it is about to hand the engine. That retires its 8 MiB refusal to
judge: a small corpus is now a regime it can price rather than one it has to decline, and
it renders a verdict in both.

## The mint can be fooled, and was

The two columns read **identical to four decimal places** on the first attempt, which
looks exactly like the finding "residency does not matter on this silicon" — and was read
that way for an afternoon. It was not a finding. `mint` was aimed at this repository,
which is 0.5 MiB, so the 64 MiB request and the 1 MiB request both returned every byte in
the tree: the same bytes, timed twice.

`examples/mint.rs` now **refuses** a corpus under 32 MiB rather than printing a row, and
refuses a cache slice larger than an eighth of the whole. A row is a claim about a memory
system, and a mint that never reached memory has no business making one. It also warns
when either regime-indexed coefficient comes out inverted, since the direction is physics
and a run that contradicts it measured something else.

The same trap manufactured a 30% "drift" that briefly looked like the shipped row going
stale: a cache-resident mint reads `dfa_excursion` near 6.7-8.1 against the row's 9.75.
The row was right and the re-mint was measuring the other regime. The memory-resident
column of the new row lands at 9.751 against the old 9.7495 — the cross-check that says
the cache column is new information rather than a re-labeled old number.
