# `sheng/src/`

One deep package behind one type. `Sieve` in `lib.rs` is the surface an ordinary
caller needs; everything else here is a stage of the pipeline that builds it, and
none of them is independently useful for filtering a document. They stay `pub`
regardless, for two reasons: `Policy` exposes their measured types (`Calibration`,
`Chain`, `Kernel`) so a caller can override them, and `examples/{mint,bench,skip,
survey}.rs` are calibration instruments that read the pipeline's own intermediate
stages directly rather than duplicating them.

The pipeline runs in one direction, each stage narrowing what the next can consider:

| module           | stage                                                                         |
| ---------------- | ----------------------------------------------------------------------------- |
| `projection.rs`  | a `dense::DFA` becomes reachable core states + the exact byte partition       |
| `lattice.rs`     | the SP-partition closure over that core, and which quotients to conjoin       |
| `shuffle.rs`     | the register kernel that runs a quotient: dispatch, composition, scalar reference |
| `arch/`          | the unsafe NEON, AVX-512, AVX2, SSSE3 and SIMD128 intrinsics `shuffle.rs` and `skip.rs` dispatch into |
| `skip.rs`        | finding the next byte that leaves the start block, exactly rather than nearly |
| `prior/`         | what a byte is likely to be, given the byte before it                         |
| `selectivity.rs` | the joint (block, class) chain that predicts how often a quotient accepts     |
| `price/`         | what each kernel costs, and the inequality that decides arming                |

The split that matters is the last three. `prior` knows only about bytes, `selectivity`
only about a quotient's chain, and `price` only about measured nanoseconds — so the
byte model can be re-minted, the chain can be generalized, and the calibration can move
to new silicon without any of the three touching the others. They were one file once,
and every change to the cost model meant re-reading the Markov arithmetic to be sure
nothing had shifted underneath it.

`skip.rs` is the one stage that is optional rather than sequential. It offers the kernel
a cheaper way to cross bytes that provably cannot change the quotient's block, and
`lib.rs` prices that offer against the plain kernel per conjunct and takes the cheaper —
so a sieve may run a skip on one lane and a shuffle on the next. It has a scalar
statement of its own set membership (`find_scalar`) that the SIMD path is differentiated
against, because it is the only stage where being fast and being wrong look identical
from the outside: a skip that steps over a real escape byte silently loses a match.

`prior/` and `price/` are the two that carry **measured constants**, each stamped with
the machine or corpus and the date that produced it. Re-mint both with
`cargo run --release --example mint`; never hand-edit a coefficient.

They are stamped for different reasons, and the asymmetry is worth knowing. A price row
is nanoseconds on one machine under one load, so it can only be read by a human and never
checked — which is what `mint.yml` is, a manual workflow that prints rows for somebody to
judge. A prior is a *count*: given the same bytes it is the same floats on any runner in
any month. So `prior/minted.rs` ships four corpora minted from trees pinned by commit,
and `priors.yml` re-derives three of them on every relevant push and fails on a drifted
cell. `SOURCE`'s corpus is this repository, which is why it is the one that stays stamped
rather than checked.

Because those two are the only empirical stages, they are also the only ones a caller can
be wrong about — so `Policy` in `lib.rs` exposes exactly them: the calibration, the chains,
the byte marginals, the nominal document length. `price::MINTED` carries one row per
(architecture, kernel) pair that has actually been measured and `price::active()` resolves
the running machine against it; a machine with no row declines every pattern rather than
borrowing another machine's ratios. `shuffle::kernel()` reports which of the three kernels
dispatch chose, which is what lets the differential test prove it compared a vector path
against the scalar one instead of the scalar path against itself.

`price/` splits the same way `prior`/`selectivity`/`price` split from each other: `gate.rs`
is pure arithmetic with no measured numbers in it, `calibration.rs` is the `Calibration`
shape and its per-byte methods, and `minted.rs` is nothing but the measured rows — so
re-minting a machine touches one file, and the worth-test inequality can be read (and
tested) with no numbers in view at all. `prior/` is that same split with two files rather
than three: `mod.rs` is the model — seven classes, a 7x7 chain, why a memoryless one
misprices a run — and `minted.rs` is nothing but the four corpora it measured. `arch/` is
the same split applied to the two SIMD
kernels: `shuffle.rs` and `skip.rs` hold the portable dispatch and scalar fallbacks, while
every `unsafe` intrinsic lives behind an `arch::` module named for its instruction set,
each gated to compile only on its own target architecture.
