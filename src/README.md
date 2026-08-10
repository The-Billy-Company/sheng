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
to new silicon without any of the three touching the others.

`skip.rs` is the one stage that is optional rather than sequential: it offers the kernel
a cheaper way to cross bytes that provably cannot change the quotient's block, and
`lib.rs` prices that offer per conjunct and takes the cheaper, so a sieve may run a skip
on one lane and a shuffle on the next. Its module doc carries the exactness argument.

`prior/` and `price/` are the two that carry **measured constants**, each stamped with
the machine or corpus and the date that produced it. Re-mint both with
`cargo run --release --example mint`; never hand-edit a coefficient.

They are stamped for different reasons. A price row is nanoseconds on one machine under
one load, so it can only be read by a human and never checked — which is what `mint.yml`
is. A prior is a *count*: given the same bytes it is the same floats on any runner in any
month, so `priors.yml` re-derives the pinned corpora on every relevant push and fails on
a drifted cell. `SOURCE`'s corpus is this repository, which is why it stays stamped
rather than checked.

Every submodule splits the same way, measured numbers alone in a file of their own:
`price/` into `gate.rs`, `calibration.rs` and `minted.rs`; `prior/` into the model and
the corpora it measured; `arch/` into portable dispatch here and one `unsafe` intrinsic
module per instruction set, each gated to compile only on its own target architecture.
