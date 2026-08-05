# Sheng: A Regex Refutation Sieve

A regex proves a document innocent far more cheaply than it proves one guilty —
that asymmetry is the whole crate. Sheng builds a register-sized
over-approximation of a pattern's automaton and uses it to prove a haystack
match-free before a real engine walks it. The contract runs one way: a sieve may
pass a document that doesn't match, but may never reject one that does —
refutation is sound, confirmation is somebody else's job.

Sheng also decides whether to exist. Most patterns should never front a filter,
so the gate prices one against the engine it would sit in front of and declines
when it wouldn't pay — the common, intended outcome.

## Contents

- [Installation](#installation)
- [Platforms](#platforms)
- [Usage](#usage)
- [The Cases That Pay](#the-cases-that-pay)
- [The Wrong Tool for the Job](#the-wrong-tool-for-the-job)
- [Calibration](#calibration)
- [Layout](#layout)
- [Development](#development)
- [The Design](#the-design)
  - [Soundness](#soundness)
  - [The Parallel Kernel](#the-parallel-kernel)
  - [The Start-Block Skip](#the-start-block-skip)
  - [The Cost Gate](#the-cost-gate)
  - [The Persistence-Aware Prior](#the-persistence-aware-prior)
  - [The Excursion Coefficient](#the-excursion-coefficient)
  - [Machine Dependence](#machine-dependence)
- [Prior Art](#prior-art)
- [Problem Reports](#problem-reports)
- [Non-Negotiables](#non-negotiables)

## Installation

```bash
cargo add sheng
```

Rust 1.95+, edition 2024, Apache-2.0. Two dependencies — `regex-automata` for
the automaton, `memchr` for narrow byte searches — both already in the graph of
anyone using `regex`.

## Platforms

Six targets, x86_64 and arm64, all equally first-class: Linux (`ubuntu-24.04`,
`ubuntu-24.04-arm`), macOS (`macos-15-intel`, `macos-15`), Windows
(`windows-2025`, `windows-11-arm`).

`src/arch/` dispatches on `target_arch` alone, never the OS: one NEON kernel and
one runtime-probed SSSE3 kernel behind all six.
[`.github/workflows/native.yml`](.github/workflows/native.yml) runs every cell
on real, never-emulated silicon on every push, and re-checks the economic gate
against real source text.

## Usage

Build the sieve once, then ask it about every document:

```rust
use sheng::Sieve;

let Ok(sieve) = Sieve::new(r"#[0-9a-fA-F]{6}") else {
    return search_every_document(); // no sieve; just run the engine
};

for doc in &documents {
    if sieve.refutes(doc) {
        continue; // provably match-free - the engine never sees it
    }
    confirm_with_the_real_engine(doc);
}
```

A `BuildError` always means the same thing: run unfiltered. `NotWorthIt`, the
usual variant, carries the arithmetic that declined it — selectivity, survival
rate, both per-byte prices.

A `Sieve` is immutable with no scan state, so one instance serves every document
and thread with no cloning. Hand it a DFA you already built to share one
automaton between filter and confirming search, so the gate also prices the
rival off the engine that will actually run:

```rust
let sieve = sheng::Sieve::of_dfa(&dfa)?;
```

## The Cases That Pay

Sheng pays when one pattern crosses many documents and most don't match — a
corpus scan, a log sweep, a rule slate over a stream. The gate answers in about
half a millisecond. Three properties arm a pattern:

- **A distinctive alphabet** — `[0-9]{3}-[0-9]{4}` runs 4.42x; digits and a
  hyphen refute almost every position in prose or code.
- **A rival with no cheap accelerator** — `panic!\(` runs 1.47x on arm64; its
  `memchr` lead byte is too common to skip past.
- **Documents in the low kilobytes** — survival rises with length, so a filter
  clearing 4 KiB may keep 64 KiB on one surviving position.

A rare lead byte is the reliable, correct decline: `regex-automata` crosses a
document at 0.0158 ns/B via `memchr`, against the sieve's 0.188. Nothing that
inspects every byte beats a twelve-times-faster skip.

## The Wrong Tool for the Job

Sheng is a prefilter, not an answerer — it never says where a match is, only
that one cannot exist. Use [regex-automata](https://github.com/rust-lang/regex)
directly; this crate calls it to confirm every survivor anyway.

For searching a repository, that's `gist` — indexed, ranked, tuned for the
corpus you stand in.

## Calibration

Two measurements decide arming, neither universal: what three loops cost on this
machine, and what the bytes look like. Both live in `Policy`; override whichever
is wrong for your corpus or silicon.

```rust
let mut policy = sheng::Policy::default();
policy.len = 4096.0; // override the 64 KiB default
let sieve = sheng::Sieve::with(r"\bTODO\b", &policy);
```

Mint the rest with `cargo run --release --example mint`, pointing
`$SHENG_CORPUS` at your real bytes. An unmeasured machine gets
`price::UNMEASURED`, declines every pattern, and says so through
`BuildError::Uncalibrated` — fail-closed, since guessing another machine's
silicon is deliberately not offered.

## Layout

One deep package behind one type; each module is a pipeline stage. `Sieve` is
the surface most callers need; the stage modules stay `pub` because `Policy`
exposes their measured types for override.

- **`lib.rs`** — `Sieve`, the arming decision, `Policy`.
- **`projection.rs`** — reachable core states, the byte-class partition.
- **`lattice.rs`** — the SP-partition closure, which quotients to conjoin.
- **`shuffle.rs`** — the register kernel: NEON, SSSE3, scalar reference.
- **`skip.rs`** — the next byte that leaves the start block, exactly.
- **`selectivity.rs`** — the joint (block, class) chain that predicts `f`.
- **`prior.rs`** — what a byte is likely to be, given the byte before it.
- **`price/`** — what each kernel costs, and the inequality that decides.

## Development

```bash
cargo test --release                   # soundness, judged ungated
cargo run --release --example survey   # end to end, no armed row may lose
cargo run --release --example skip     # per-lane skip-vs-compose audit
cargo run --release --example bench    # per-stage build cost, kernel ns/byte
cargo run --release --example mint     # re-mints the prior and calibration
SHENG_NO_SKIP=1 cargo run --release --example survey   # the pre-skip baseline
SHENG_CORPUS=/path/to/your/documents cargo run --release --example mint
```

On PowerShell, set `$env:SHENG_CORPUS` in its own statement first — it doesn't
parse `VAR=value cmd`. Examples read real source, so the persistence question
isn't answered by a synthetic generator, and every constant carries its machine,
kernel, and date.

`survey` is a gate — every armed row must land above 1.000x — not a report;
`bench` is the report, split into four stages so a regression lands on a named
one. Below 8 MiB, `survey` declines to judge at all, since a cache-resident
corpus lets the engine's own accelerator run at tens of GB/s with no calibration
to describe it.

## The Design

Why the approximation is sound, why the kernel is fast, why the gate refuses so
often.

### Soundness

Partition the DFA's states so the partition is closed under the transition
function: if `p ≡ q` then `δ(p,b) ≡ δ(q,b)` for every byte `b`. That's the
_substitution property_; SP partitions form a lattice under refinement
(Hartmanis & Stearns, _Algebraic Structure Theory of Sequential Machines_,
Prentice-Hall, 1966, ch. 2).

Every SP partition induces a quotient automaton on the blocks. Marking a block
accepting whenever any member accepts makes it recognize a superset of the
language:

> the real automaton reaches an accepting state ⟹ every quotient does.

Contrapositive: if a quotient accepts nowhere in the haystack, no match exists —
a survivor proves nothing, a rejection proves everything.

A conjunction of quotients is sound and strictly stronger than either — capped
at 2, since a third rarely earns its shuffle. Every partition is re-derived and
re-checked; one that isn't actually closed is discarded, not shipped.

### The Parallel Kernel

Langdale's kernel holds state in the register, so every shuffle needs the
previous one's answer — one byte per shuffle latency forever, 2 cycles a byte on
an M4, and no unrolling shortens that chain.

Hold the transition function in the register instead, seeded with the identity
`[0,1,…,15]`. The same single shuffle per byte now composes: after a run of
bytes, lane `i` is the block that run would reach from block `i` — all sixteen
answers for the price of one.

That frees the haystack to split into four independent slices running four
chains at once, four because that's about how many it takes to saturate the
shuffle port. Geomean over eight patterns on an M4: 0.346 to 0.132 ns/byte, 2.6x
— and every size gets faster, 1.22x at 64 B up to 2.97x at 64 KiB, since a
document shorter than one slice re-derives its own stride instead of falling
back to scalar.

Composition gives a per-lane running max, so collapsing the slices reads the
true maximum over the chunk — the parallel kernel refutes exactly what the
scalar reference does. Two rejected alternatives: reading all sixteen lanes is
sound but treats hypothetical scans as real, so nothing refutes; deleting the
max via a self-loop measured inside the noise (0.131 vs 0.134 ns/byte) for the
cost of a second trapping form of every quotient.

### The Start-Block Skip

At 0.131 ns/byte the loop is load-port bound — row and haystack, two loads per
byte — so the only move left is to stop reading bytes.

A quotient sits in its start block until a byte _escapes_ it; every self-loop
byte between is provably a no-op. Unanchored patterns live there almost entirely
— 98.2% of bytes for `WalletService`, 99.2% for `#[0-9a-fA-F]{6}` — so
`skip::Skip` finds the next escape directly and jumps to it, via two instruments
chosen by how many byte values leave the block:

- **1-3 values** — `memchr`, already in the graph, the best-tuned SIMD byte
  search around.
- **4-128 ASCII values** — a nibble classifier: `lo[b & 0xF]` carries one bit
  per high nibble, `hi[b >> 4]` selects it, product nonzero for members.

The classifier is Wojciech Muła's [SIMDized check which bytes are in a
set](http://0x80.pl/articles/simd-byte-lookup.html) (2018), shipped in Hyperscan
as `shufti`. Sheng covers `0x00..=0x7F` only, one bitmap pair against Muła's
two; a wider or higher set is refused outright (`Skip::of` returns `None`),
since a false "not a member" would skip a real transition and turn a sound
refutation into a missed match.

The choice is per lane, since skipping frequently loses: 8.8-11x against a
one-byte escape set, 0.25x against a three-way alternation. `Lane::plan` prices
both and takes the cheaper, matching the measured winner 8 of 8 on the survey
slate, priced by a coefficient minted specifically for it, `skip_excursion`,
since the engine's own `dfa_excursion` under-predicts this path. Differentiated
against a scalar reference three ways: every byte value against every set shape,
2048 pseudo-random ASCII sets, and a planted escape at every offset of every
length 1-72.

### The Cost Gate

The filter, built first, made everything slower: geomean 0.230x, a 4.3x
regression across thirteen patterns, because it armed on every pattern that
could harvest a quotient.

Position rejection isn't document rejection. Retiring 99% of byte positions
sounds decisive; one survivor still drags the whole buffer into verification —
over 4 KiB at `f = 0.01`, `1 — (1-f)^4096` is 99.99% survival. What matters is
documents kept, which rises far faster than the per-position rate falls:
`[0-9]{4}-[0-9]{2}-[0-9]{2}` rejects 99.03% of positions and still keeps 80% of
documents, because dates cluster.

The rival's price decides more often than selectivity does, so the gate is an
inequality between two measured costs:

```text
sieve  +  (1 - (1-f)^len) * rival   <   rival
```

Every term is absolute ns/byte, and the rival's term is read from
`Automaton::accelerator` on the engine's own start state — the crate asks what
it intends to skip, and prices that.

### The Persistence-Aware Prior

Predicting `f` without a calibration haystack is the hard half — sampling the
document has already paid for the scan. The estimate comes only from the
quotient's own Markov chain.

An independent-draw chain prices a `k`-byte class run as `p^k`, but real text
isn't memoryless. Measured over 64 MiB of this repository (marginal probability,
repeat probability, ratio):

- **`Lower`** — 0.5703, 0.7683, 1.3x.
- **`Digit`** — 0.0186, 0.3863, 20.8x.
- **`High`** — 0.0139, 0.9167, 66.0x.

A digit is 21x likelier to follow a digit than at random, so the naive model
under-prices a forty-digit run by `20.8^40` — a filter believing it rejects
everything while rejecting nothing.

The fix carries the byte's class in the chain's state: (block, class) pairs, 112
states, a 7x7 matrix, collapsing exactly to the naive chain under a memoryless
prior. `Prior::Text` stays as that superseded case on purpose, so its error
stays measurable.

That chain was also the entire build cost: 512 power iterations over 112 states,
99.9% of a 44-to-75 ms build. It factors — the byte draw depends only on the
previous class — so aggregating bytes into class edges once at construction
visits only the transitions that exist, no approximation added. Measured on an
M4: `WalletService` build time falls 64.1 to 0.55 ms, `#[0-9a-fA-F]{6}` 72.6 to
0.34 ms — 116x to 278x across the five-pattern slate. One thing didn't survive:
exiting early once the distribution looked settled, since the accepting mass can
be `1e-29` and still climbing after the bulk has stopped moving — the iteration
count is now fixed.

### The Excursion Coefficient

`dfa_excursion` is the one term we couldn't derive: an accelerated engine pays
for a whole DFA excursion when `memchr` trips, not one byte, and the restart
dominates. Omitting it under-priced a common-byte accelerator by 8x.

We timed eleven accelerated patterns spanning two orders of magnitude of
lead-byte frequency and inverted the blend for `E`. Read at class resolution,
they spanned 3.6 to 35.2, a tenfold disagreement; read from a per-byte table,
they collapsed to roughly 7.5-13, mean ~10.3, stable across re-mints — evidence
the coefficient is real. The slate now classifies perfectly: six armed, seven
declined, every declined row independently measured below 1.0x when forced to
arm.

The armed-row geomean can mislead: 2.21x with the skip kernel on, 2.83x with it
off (`SHENG_NO_SKIP=1`). The skip is still the improvement — it arms two
patterns that couldn't pay for themselves before, both marginal (`WalletService`
1.16x, `foo[^\n]*bar` 1.09x), pulling the average down. Held to the four that
armed either way, the slate goes 2.83x to 3.11x, nearly all of it
`[0-9]{3}-[0-9]{4}` moving 3.21x to 4.42x.

### Machine Dependence

Three layers could tie to the measuring machine; only one really does. The
mathematics and kernel are pure arithmetic, differentiated across 30,000 mutated
haystacks against the scalar reference, run as a standing proof on all six
native targets in [`.github/workflows/native.yml`](.github/workflows/native.yml)
on every push.

The clock is provably irrelevant: multiply every `Calibration` coefficient by
any positive constant and no decision moves, since the factor cancels on both
sides of the arming inequality. A loaded laptop, a throttled core, a 3-versus-5
GHz gap all decide identically.

What survives scaling is three dimensionless ratios, minted from this crate's
own source tree:

- **`skip/walk`** — 0.013 arm64, 0.010 x86_64.
- **`sieve/walk`** — 0.149 arm64, 0.175 x86_64.
- **Excursion** — 9.7 arm64, 11.6 x86_64.

They differ less than instinct suggests once both sides run the same kernel —
walk cost is nearly identical, 1.26 against 1.25 ns/B. Re-minted together on
2026-08-03 after `shuffle` moved to four parallel slices, `panic!\(` now arms on
both: 1.329x arm64, 1.319x x86_64.

`price::MINTED` holds one row per (architecture, kernel) pair measured so far,
keyed deliberately without `os` — Windows shares both rows above, since nothing
upstream varies by it. Anything unmeasured declines, naming the missing
measurement.

The corpus is the real limit: shipped priors describe a polyglot source tree,
and prose or minified JS are different byte processes.
[Calibration](#calibration) overrides it; nothing here is hardcoded.

## Prior Art

This crate is a Rust port of one rung of our Zig engine
[irregex](https://github.com/The-Billy-Company/irregex) — `quotient.zig` and
`sheng.zig` in its `sieve/` package. The codename is the kernel's; it's what
makes the idea affordable.

The contract — over-approximate, reject early, verify survivors exactly — is
prior art at least three times over:

- **Luchaup, De Carli, Jha & Bach**, _Deep packet inspection with DFA-trees and
  parametrized language overapproximation_, INFOCOM 2014
  ([10.1109/INFOCOM.2014.6847977](https://doi.org/10.1109/INFOCOM.2014.6847977))
  — Definition 7 is exactly `|D'| < |D|` with `L(D) ⊆ L(D')`, calling shrunk
  DFAs "a special case of quotient automaton." Measured 4.7x, and +26% when
  nothing rejects — the hazard our cost gate exists to avoid.
- **Češka, Havlena, Holík, Lengál & Vojnar**, _Approximate reduction of finite
  automata for high-speed network intrusion detection_
  ([arXiv:1904.10786](https://arxiv.org/abs/1904.10786), 2019) — a cascade of
  small over-approximating NFAs chosen by a probabilistic traffic model.
- **Hyperscan's `HS_FLAG_PREFILTER`** — matches are a superset, and the caller
  confirms with an exact matcher.

The kernel is Langdale's Sheng (2018,
[branchfree.org](https://branchfree.org/2018/05/25/say-hello-to-my-little-friend-sheng-a-small-but-fast-deterministic-finite-automaton/),
shipped in Hyperscan) — a DFA of ≤16 states in one register, stepped with a
single `pshufb` / `vqtbl1q_u8`. We point it at an over-approximating quotient
instead of the real automaton.

Parallelizing it is Mytkowicz, Musuvathi & Schulte's _Data-parallel finite-state
machines_, ASPLOS 2014
([10.1145/2541940.2541988](https://doi.org/10.1145/2541940.2541988)) — compute
transitions from all possible states at once as a gather, implemented with
`_mm_shuffle_epi8`. Their overhead grows with state count; ours doesn't, since
the same sixteen-block cap that makes the filter sound also makes the
enumeration free.

Three claims stay narrow: the SP-lattice harvest as the approximation's source,
the ≤16-state register-resident conjunction, and the training-free gate. Two
pieces of the gate aren't ports either, named as open residuals by the Zig
rung's own bench: the persistence-aware prior (the Zig gate's memoryless prior
under-prices a 40-byte run by seventeen orders of magnitude, arming one pattern
into a measured 0.89x loss), and the accel-aware rival term (the Zig inequality
prices the fronted machine at full-scan cost; this one prices the skip the
engine itself intends to take).

## Problem Reports

A wrong answer — `refutes` returning `true` for a document that matches — is a
soundness bug and outranks everything else here. Include the pattern, the bytes,
and the output of `shuffle::kernel()`.

Bugs and vulnerabilities go through [SECURITY.md](SECURITY.md) and
[CONTRIBUTING.md](CONTRIBUTING.md).

Two neighbors own surfaces easy to mistake for this one: pattern meaning —
parsing, Unicode, match semantics — belongs to
[regex-automata](https://github.com/rust-lang/regex), which this crate reads but
never parses itself; the Zig original belongs to
[irregex](https://github.com/The-Billy-Company/irregex).

## Non-Negotiables

The differential harness tests every pattern that harvests a quotient, not just
the minority the economics arm — testing only armed ones would shrink the suite
every time the gate got stricter, exactly backwards, since a false reject is a
missed match the moment a quotient exists. `Gate::Ungated` exists for that
reason alone.

The gate declines most patterns, and that's the design working. A prefilter's
honesty is measured by how often it refuses.
