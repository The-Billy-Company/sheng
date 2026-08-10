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
  - [Everywhere else](#everywhere-else)
- [Usage](#usage)
- [The Cases That Pay](#the-cases-that-pay)
  - [When the confirm is not a regex](#when-the-confirm-is-not-a-regex)
  - [When the rival is the engine](#when-the-rival-is-the-engine)
  - [A slate is not a pattern](#a-slate-is-not-a-pattern)
  - [Bounded repeats no longer refuse unpriced](#bounded-repeats-no-longer-refuse-unpriced)
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

Rust 1.95+, edition 2024, Apache-2.0. Two dependencies by default —
`regex-automata` for the automaton, `memchr` for narrow byte searches — both
already in the graph of anyone using `regex`.

Neither is on the scan path, and the feature set says so rather than a paragraph:

```bash
cargo add sheng --no-default-features   # no_std + alloc, memchr only
```

`--no-default-features` is a `no_std` build. What goes away is the parser, so
`Sieve::new` goes with it and [`Sieve::of_dfa`][dfa] over any automaton
satisfying the `Dfa` trait is the way in. What stays is everything that runs:
the projection, the lattice harvest, the selectivity model, the vector kernels,
and the arming gate. There is no `powf`, no `libm`, and no math library behind
either — every float operation in the crate is `+ - * /` and a comparison — and
the x86 feature probes read `CPUID` and `XCR0` directly rather than through
`std::arch::is_x86_feature_detected!`, so nothing of sheng's own is lost. An
allocator is still required; the transition tables are `Vec`-shaped.

Both features are on by default, and each adds only what its name says:

- **`regex-automata`** — the parser, and the `Dfa` impl for the engine that runs
  the confirming search.
- **`std`** — `memchr`'s AVX2 runtime dispatch, and `regex-automata`'s own
  `std` leg.

[dfa]: https://docs.rs/sheng/latest/sheng/struct.Sieve.html#method.of_dfa

## Platforms

Six targets, x86_64 and arm64, all equally first-class: Linux, macOS, and
Windows on each.

`src/arch/` dispatches on `target_arch` alone, never the OS: one NEON kernel and
three runtime-probed x86_64 kernels — AVX-512, AVX2, SSSE3 — behind all six.
[`.github/workflows/native.yml`](.github/workflows/native.yml) runs every cell
on real, never-emulated silicon on every push, and re-checks the economic gate
against real source text.

Which kernel a machine _runs_ is a narrower question than which it can execute,
and deliberately so: dispatch elects the **cheapest kernel this machine's own
calibration row measured**, so an unminted kernel is inert rather than trusted.
Cheapest, not widest — a wider register is a guess about speed, and on the one
runner offering all three x86_64 rungs the mint refuted it: the 64-byte shuffle
is the slowest of the three, perfectly correct and simply not elected. A kernel
nobody has priced on a given machine is named in [`price::DORMANT`][dormant] with
the reason, and its entries are deleted by
[`.github/workflows/mint.yml`](.github/workflows/mint.yml) rather than by
argument. See [Calibration](#calibration).

`wasm32` is a seventh target and the one exception to the paragraph above, since
a guest has no `CPUID` to probe: `-C target-feature=+simd128` chooses between the
SIMD128 kernel and the scalar one at compile time. CI runs the differential under
`wasmtime`. It has no minted row either — and a row there would be a claim about
the runtime and the host under it as much as about the guest — so a sieve
declines on `wasm32` unless the caller supplies its own `Calibration`.

### Everywhere else

Nothing above is a portability claim, and this is the part that used to read like
one. The scan path compiles and runs on any target Rust supports — `riscv64`,
`powerpc64`, `s390x`, `loongarch64` — through `Kernel::Scalar`, which is the
reference composition pass and needs no vector instruction at all. What those
machines lacked was not a kernel but a **price**: no row in `price::MINTED`, so
`BuildError::Uncalibrated` on every pattern, so an inert dependency.

That is now a call rather than a wait for someone to ship a row:

```rust,no_run
# let corpus: Vec<Vec<u8>> = Vec::new();
use sheng::price::Calibration;
use sheng::{Policy, Screen};

// A sample of the documents this process actually searches.
let sample: Vec<&[u8]> = corpus.iter().map(Vec::as_slice).collect();
let mine = Calibration::measure(&sample)?;

let mut policy = Policy::new(mine.regime().expect("a measured row names one regime"));
policy.calibration = mine;
let screen = Screen::with(r"(?-u)AKIA[0-9A-Z]{16}", &policy)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

A row taken this way is _better_ evidence than a shipped one on two counts — it
describes this machine rather than one that shares its `(os, arch, kernel)`
triple, and it is measured over the caller's own bytes rather than one of four
shipped corpora. What it gives up is reproducibility, since a shipped row was
taken on an idle CI runner. `Bench` refuses rather than guesses when the sample
is too small to time honestly, which is the case that would otherwise return
a plausible row full of clock granularity.

`cargo run --release --example mint` is the same measurement formatted for pasting
into `price::MINTED`, which is how one caller's fix becomes everybody's. Both go
through `price::Bench`; the example adds only the `const` literal and the
per-kernel sweep.

[dormant]: https://docs.rs/sheng/latest/sheng/price/constant.DORMANT.html

## Usage

`Screen` is the front door, and it cannot decline. It builds a sieve, keeps it if
one pays, and runs the engine alone if not:

```rust
# let documents: [&[u8]; 0] = [];
use sheng::{Residency, Screen};

// `Residency` is the one input the crate cannot probe and refuses to guess:
// whether these bytes arrive from cache or from main memory. See Calibration.
let screen = Screen::new(r"(?-u)#[0-9a-fA-F]{6}", Residency::Memory)?;

for doc in documents {
    if screen.is_match(doc) {
        // ... handle the document that might match ...
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Identical in answer to running `regex-automata` alone; the only difference is how
much work it does to get there. The one error it returns is a pattern that does
not parse, so the ordinary decline — this filter would not pay on this machine —
costs the caller no code at all. `screen.declined()` still hands over the refusal
and its arithmetic for anyone who wants to read it.

`Sieve` is the same thing without the fallback, for a caller who already has a
matcher to fall back to:

```rust
# fn confirm_with_the_real_engine(_doc: &[u8]) {}
# let documents: [&[u8]; 0] = [];
use sheng::{Residency, Sieve};

let Ok(sieve) = Sieve::new(r"#[0-9a-fA-F]{6}", Residency::Memory) else {
    return; // no sieve; just run the engine over everything
};

for doc in documents {
    if sieve.refutes(doc) {
        continue; // provably match-free - the engine never sees it
    }
    confirm_with_the_real_engine(doc);
}
```

A `BuildError` always means the same thing: run unfiltered. `NotWorthIt`, the
usual variant, carries the arithmetic that declined it — selectivity, survival
rate, both per-byte prices, and the speedup no price could have passed.

A `Sieve` is immutable with no scan state, so one instance serves every document
and thread with no cloning. Hand it a DFA you already built to share one
automaton between filter and confirming search, so the gate also prices the
rival off the engine that will actually run:

```rust
# use regex_automata::dfa::dense;
# let dfa = dense::DFA::new(r"#[0-9a-fA-F]{6}").unwrap();
use sheng::{Residency, Sieve};

let sieve = Sieve::of_dfa(&dfa, Residency::Memory);
```

## The Cases That Pay

Sheng pays when one pattern crosses many documents and most don't match — a
corpus scan, a log sweep, a rule slate over a stream. The gate answers in a
fraction of a millisecond, and what it answers turns on three things in this
order: **what the caller would otherwise have done**, then how short the records
are, and only then how good the filter is. The first is where most of the room
is, and it is also where most of the mistakes are.

### When the confirm is not a regex

A refutation's product is not a faster scan. It is a proof that a document needs
**no further work**, and what that is worth is set entirely by what the further
work would have been. So a confirm that extracts text from a PDF, embeds the
document, or fetches it over a network — hundreds to thousands of times a DFA
walk — sounds like it should change everything, and `Policy::rival` is where you
would say so.

It almost never does, and the reason is the most useful thing on this page. The
work a refutation saves is the work you would _otherwise have done_, and a caller
holding a regex would not put every document through an OCR pass. They would run
the engine first — it decides the same question exactly, for a hundredth of the
price — and pay the extraction only on documents that truly match. Comparing a
sieve against the OCR is comparing it against a pipeline nobody runs. So the gate
takes the cheaper of the two, and `Policy::bypass` is where you say what you have:

```rust
use sheng::{Bypass, Policy, Residency, Rival, Sieve};

let mut policy = Policy::new(Residency::Memory);
policy.rival = Rival::Walks(512.0); // a survivor costs ~512 DFA walks per byte
// ...but the engine can screen for it, which is the default and the usual truth:
assert_eq!(policy.bypass, Bypass::Engines);
let sieve = Sieve::with(r"(?-u)AKIA[0-9A-Z]{16}", &policy);
```

`cargo run --release --example census` sweeps 31 patterns people really grep for
and prints all of this. On the machine this paragraph was written on, **11 of 31
arm in front of the engine, and 11 in front of that 512-walk confirm** — the same
eleven, because the engine is still there to run first. Take the screen away with
`Bypass::Absent` and **24 of 31** arm.

That last number is the honest value of a costly rival, and it belongs only to a
caller who genuinely cannot decide the question more cheaply where the sieve runs.
Screening packets against rules whose matches only exist in a reassembled flow is
the case; "my confirm is slow" is not.

Nothing here manufactures selectivity either. As the rival's price grows the
inequality converges on `survival * (1 + MARGIN) < 1`, so `CostFact::ceiling` —
`1 / survival` — is the most any price or slate size can ever reach, and a filter
keeping more than `1/(1 + MARGIN)` of documents is finished. Of the seven that
still declined above, six are terminal in exactly that sense; the census prints
the split, and `NotWorthIt` carries the ceiling so a single decline says whether
tuning is worth the afternoon.

Costs to check yourself before reaching for this: a walk is 1.3–2.1 ns/byte on
the minted machines, so zstd or gzip (1–3 ns/byte), AES with hardware support
(under 1), and JSON parsing (1–3) are all _within a small multiple of the
engine_ and change nothing. `Rival` and `Bypass` document the arithmetic.

### When the rival is the engine

Then the bar is `memchr` and most patterns correctly lose to it. What arms:

- **No usable literal, and an alphabet that refutes.** `[A-Z][a-z]+Service` and
  `(alpha|beta|gamma)` are `examples/survey.rs`'s decided winners over 31 MiB of
  real source.
- **A rival with no cheap accelerator.** `panic!\(` is the marginal face of
  this: its `memchr` lead byte is common enough that the sieve competes, and
  close enough that the survey often cannot separate the two arms at all.
- **Documents in the low kilobytes.** Survival rises with length, so a filter
  that clears a short document may keep a long one on a single surviving
  position.

A rare lead byte is the reliable, correct decline: `regex-automata` can cross a
document via `memchr` an order of magnitude faster than the sieve walks it.
Nothing that inspects every byte beats a skip that cheap. A distinctive alphabet
is _not_ on its own enough to overturn that — `[0-9]{3}-[0-9]{4}` refutes almost
every position in prose or code and still declines, because rejecting positions
is not rejecting documents and the digits cluster where dates and versions are.

### A slate is not a pattern

Many rules over each document is a different economic proposition, and it takes
a term in the gate to say so. The pre-pass is paid once per document while
verification is paid once per pattern, so with `n` rivals the inequality divides
through to

```text
(sieve/n  +  survival * rival) * (1 + MARGIN)   <   rival
```

and the sieve's own price — what declines most near-parity patterns — falls away
as the slate grows. `Policy::rivals` is where the count goes. Two obligations come
with it and neither is checkable from inside the crate: the sieve has to be built
from an automaton whose language contains **every** rule's —
`Sieve::of_superset_with` is where you hand one over — and the priced rival
should be the _cheapest_ of
them, since underestimating the rival can only make the sieve decline.

This is the workload the prior art below is about, and it is the one this crate
described as its best case for a long time without ever measuring it. Measured
(`cargo test --release --test slate -- --nocapture`), the term is real and it is
bounded three ways, all worth knowing before you plan around it.

**If the rules have literals, the union is already the answer.** Sixty-four
literal-prefixed rules measure 11.96 ns/B as sixty-four separate engines and
**0.12 as one union** — the fan-out almost exactly, because the union keeps a
multi-literal accelerator and still pays one pass's price. A sieve in front of that
has nothing to retire, and `Bypass::Slate` is how you tell the gate so. What bounds
the union is construction, not throughput: the dense table goes 12.6 KiB → 4.5 MiB
→ 65 MiB at 1, 64 and 256 rules, and the build 0.2 ms → 0.75 s → 114 s, with no
determinization at all past 256 inside a gibibyte.

**A slate's own union stops being sieveable almost immediately.** One quotient has
to over-approximate every member at once. Over eight literal-free rules of the kind
a secret scanner is made of, the union's reachable core passes
`MAX_CORE_STATES` by the seventh, and the lattice stops finding a register-sized
closed partition at the _second_. A 16-block quotient of 1,200 rules is not a
filter that lost on price; it is a filter that does not exist. What does exist
is a deliberately coarse skeleton of one _family_ —
`[0-9]+[-./:][0-9]+[-./:][0-9]+` contains every SSN, card number,
date, timestamp and version string in nine states rather than the several hundred
their union needs. Choosing one is a modeling problem the crate cannot do for you,
which is exactly why `of_superset_with` takes an automaton and not a pattern list.

**Every slate size converges on the same ceiling.** The fan-out removes `sieve/n`
and touches nothing else, so the arithmetic tends to `1 / survival` and no rule
count passes it. That makes **record length**, not slate size, the term that
decides: survival compounds over positions, so the family skeleton above is worth
at most 11.3x over 256-byte records, 3.2x over 1 KiB, and 1.00x over 16 KiB — the
same filter, the same slate, three different answers. The slate regime is packets,
log lines and short records. Over large documents there is nothing left to win.

### Bounded repeats no longer refuse unpriced

A counted repeat spends a DFA state per count, so `AKIA[0-9A-Z]{16}` put the
reachable core past `MAX_CORE_STATES` and was refused before a single coefficient
was read. That is the worst kind of refusal — a _ceiling_ rather than a verdict,
reached for a reason that has nothing to do with whether a filter would pay.

`Policy::relax` (on by default) relaxes those bounds before projecting, which is
sound in the one direction that matters: dropping a bound yields a **superset**
language, and a sieve built from a superset still never rejects a document that
matches. The strict automaton still prices the rival and still confirms every
survivor, and the two candidates are priced against each other so relaxation is
never taken when it costs more selectivity than it buys structure.

Measured over the census population: **14 of 31 patterns were refused
structurally with relaxation off, 1 with it on.** `cargo run --release --example
census` prints that pair, so the claim is an instrument reading rather than a
recollection.

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

A third input is nobody's measurement, so `Policy` has no `Default` and
`Residency` is an argument instead. Whether the haystacks arrive from cache or
from memory changes which patterns pay by a large factor — the engine's `memchr`
is cheapest exactly where the sieve is least competitive — and guessing it
silently is how a pattern can arm on a memory-resident mint and lose hard on a
cache-resident corpus. A caller states it or gets no sieve.

```rust
# use sheng::{Policy, Residency, Sieve};
let mut policy = Policy::new(Residency::Memory);
policy.len = 65_536.0; // documents larger than the 4 KiB nominal
let sieve = Sieve::with(r"\bTODO\b", &policy);
```

An unmeasured machine gets `price::UNMEASURED`, declines every pattern, and says
so through `BuildError::Uncalibrated` — fail-closed, since guessing another
machine's silicon is deliberately not offered.

It is also the one refusal a caller can lift from where they stand.
`Calibration::measure(&docs)` takes a row over the caller's own documents in
seconds and goes straight into `Policy::calibration`; see
[Everywhere else](#everywhere-else). `cargo run --release --example mint` is the
same measurement with `$SHENG_CORPUS` pointed at your real bytes, formatted as a
`const` to paste — which is what turns one machine's fix into a shipped row. Both
call `price::Bench`, so the coefficient a runtime row measures and the one a
pasted row publishes cannot drift apart.

One run prints a row for every kernel the silicon can execute, not just the one
dispatch chose, and that is what makes a new instruction set reachable at all:
since dispatch declines a kernel with no row, a mint that followed dispatch would
be waiting on the measurement the measurement was waiting on. Pasting a row in is
what wakes a kernel, and it comes with a deletion — `price::DORMANT` names the
same machine and kernel and the reason, held to `price::MINTED` in both
directions by a test, so a row landed there fails the build until the line there
is gone.

## Layout

One deep package behind one type; each module is a pipeline stage. `Sieve` is
the surface most callers need; the stage modules stay `pub` because `Policy`
exposes their measured types for override.

- **`lib.rs`** — `Sieve`, the arming decision, `Policy`.
- **`projection.rs`** — reachable core states, the byte-class partition.
- **`lattice.rs`** — the SP-partition closure, which quotients to conjoin.
- **`shuffle.rs`** — the register kernel, over `arch/`'s NEON, AVX-512, AVX2,
  SSSE3, SIMD128 and a scalar reference.
- **`skip.rs`** — the next byte that leaves the start block, exactly.
- **`selectivity.rs`** — the joint (block, class) chain that predicts `f`.
- **`prior/`** — what a byte is likely to be, given the byte before it.
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
isn't answered by a synthetic generator, and every minted constant carries its
machine, kernel, and date.

`survey` is a gate — every armed row must land above unity — not a report;
`bench` is the report, split into four stages so a regression lands on a named
one. `survey` reads the regime off the corpus rather than taking it as a flag,
declaring `Residency::Cache` for a small working set and `Residency::Memory`
above last-level cache, so a small tree and a large one exercise two columns of
one calibration. It used to refuse small corpora outright, since a per-byte
price measured against memory cannot describe a corpus that never reads from
memory; the refusal now survives only for a regime this machine has no minted
column for, which is a statement about the mint rather than about the corpus.

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
at `MAX_CONJUNCTS`, since a third rarely earns its shuffle. Every
partition is re-derived and re-checked; one that isn't actually closed is
discarded, not shipped.

### The Parallel Kernel

Langdale's kernel holds state in the register, so every shuffle needs the
previous one's answer — one byte per shuffle latency forever, and no unrolling
shortens that chain.

Hold the transition function in the register instead, seeded with the identity
`[0,1,…,15]`. The same single shuffle per byte now composes: after a run of
bytes, lane `i` is the block that run would reach from block `i` — all sixteen
answers for the price of one.

That frees the haystack to split into independent slices running that many
chains at once, enough of them to saturate the shuffle port. The parallel form
is several times the serial one across the survey slate, and every size gets
faster, since a document shorter than one slice re-derives its own stride
instead of falling back to scalar.

Composition gives a per-lane running max, so collapsing the slices reads the
true maximum over the chunk — the parallel kernel refutes exactly what the
scalar reference does. `shuffle.rs` records the two alternatives that were
measured and rejected.

### The Start-Block Skip

Once the composition loop is load-port bound — row and haystack, two loads per
byte — the only move left is to stop reading bytes.

A quotient sits in its start block until a byte _escapes_ it; every self-loop
byte between is provably a no-op. Unanchored patterns live there for nearly all
of a typical document — so `skip::Skip` finds the next escape directly and jumps
to it, via two instruments chosen by how many byte values leave the block:

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

The choice is per lane, since skipping frequently loses: a one-byte escape set
can be an order of magnitude win, a three-way alternation a clear loss.
`Lane::plan` prices both and takes the cheaper, by a coefficient minted
specifically for it — `skip_excursion`, since the engine's own `dfa_excursion`
under-predicts this path. Every instrument is differentiated against a scalar
reference over every byte value, every set shape, and a planted escape at every
offset.

### The Cost Gate

Position rejection isn't document rejection. Retiring most byte positions sounds
decisive; one survivor still drags the whole buffer into verification — over a
few kilobytes at a modest fallthrough, nearly every document survives. What
matters is documents kept, which rises far faster than the per-position rate
falls: a date-shaped pattern can reject almost every position and still keep
most documents, because dates cluster.

The rival's price decides more often than selectivity does, so the gate is an
inequality between two measured costs — where the cost of _not_ filtering is
whatever the caller would really have paid, not whatever the confirm costs:

```text
alternative = min(rivals * rival, bypass)
(sieve  +  (1 - (1-f)^len) * alternative) * (1 + MARGIN)   <   alternative
```

Every term is absolute ns/byte, and both prices are read from `Dfa::accelerator`
on the engine's own start state — the crate asks what it intends to skip, and
prices that. The `min` is the part worth staring at: a caller who can settle the
question with an engine will, so an expensive `Rival` in front of a live
`Bypass::Engines` is inert by construction and cannot argue a sieve into
existence. `Bypass::Absent` is how a caller states that no such shortcut exists
where the sieve runs, and it is the only thing that makes a costly confirm
load-bearing.

Whatever the prices, `1 / survival` bounds the whole thing, so `CostFact::ceiling`
is the most any of them could ever have reached. `NotWorthIt` prints it, which
turns "this declined" into "this declined and here is whether that is arguable."

`MARGIN` sits well above unity so a modeled edge inside the mint's run-to-run
spread declines, because a verdict drawn from noisy inputs at near-parity is a
coin flip rather than a finding. `price` states the inequality's terms and names
the test that holds the whole thing scale-invariant.

### The Persistence-Aware Prior

Predicting `f` without a calibration haystack is the hard half — sampling the
document has already paid for the scan. The estimate comes only from the
quotient's own Markov chain.

An independent-draw chain prices a `k`-byte class run as `p^k`, but real text
isn't memoryless. Digits and non-ASCII bytes in particular are far more likely
to follow themselves than their marginal share suggests — so the naive model
under-prices a long digit run by many orders of magnitude, a filter believing
it rejects everything while rejecting nothing.

The fix carries the byte's class in the chain's state: (block, class) pairs over
a small class alphabet, collapsing exactly to the naive chain under a
memoryless prior. `Prior::Text` stays as that superseded case on purpose, so its
error stays measurable.

The joint chain was also the entire build cost, and it factors — the byte draw
depends only on the previous class — so aggregating bytes into class edges once
at construction drops build time by two orders of magnitude across the survey
slate with no approximation added.

### The Excursion Coefficient

`dfa_excursion` is the one term we couldn't derive: an accelerated engine pays
for a whole DFA excursion when `memchr` trips, not one byte, and the restart
dominates. Omitting it under-priced a common-byte accelerator by nearly an
order of magnitude.

We timed a slate of accelerated patterns spanning two orders of magnitude of
lead-byte frequency and inverted the blend for `E`. Read at class resolution,
the inverted values disagreed by about tenfold; read from a per-byte table,
they collapse into a narrow band stable across re-mints — evidence the
coefficient is real. The survey slate now classifies cleanly: every declined
row independently measures below unity when forced to arm.

### Machine Dependence

Three layers could tie to the measuring machine; only one really does. The
mathematics and kernel are pure arithmetic, differentiated across a large
battery of mutated haystacks against the scalar reference, run as a standing
proof on all six native targets in
[`.github/workflows/native.yml`](.github/workflows/native.yml) on every push.

The clock is provably irrelevant: multiply every `Calibration` coefficient by
any positive constant and no decision moves, since the factor cancels on both
sides of the arming inequality. A loaded laptop, a throttled core, a wide clock
gap all decide identically.

What survives scaling is three dimensionless ratios, minted from real source:

- **`skip/walk`** — how cheap `memchr` is against a dense walk.
- **`sieve/walk`** — how cheap the composition kernel is against that walk.
- **Excursion** — how many walk-bytes an accelerator restart costs.

They differ less than instinct suggests once both sides run the same kernel —
walk cost is nearly identical across the shipped rows.

`price::MINTED` holds one row per (os, architecture, kernel) triple measured so
far, covering all six native machines. The `os` column is there because it was
earned: keyed on the pair alone, three of the six native legs ran on a row
minted on a fourth machine and were caught arming a pattern that then lost
against real source text. `MINTED`'s own documentation carries the figures.
Anything unmeasured declines, naming the missing measurement.

Four corpora are minted rather than one, because a byte prior is a claim about a
corpus and shipping only source text priced everyone else's bytes under a model
of our Rust. They disagree at the coarsest level — indentation makes a space the
likeliest thing to follow a space in a code tree, and in prose it is the least
likely — so the default sweeps all four and takes the worst, and a caller who
knows they are searching logs narrows to `Prior::Log` for a better-informed and
looser decision. Each names its corpus, then the one shape that makes it differ:

- **`Prior::Source`** — a polyglot code tree; `Digit` repeats far above its
  marginal share.
- **`Prior::Prose`** — literary English; `Space` is _anti_-persistent.
- **`Prior::Json`** — `simdjson-data`; nothing is rare, most classes persist
  heavily.
- **`Prior::Log`** — `loghub` across many systems; `Punct` alternates rather
  than clusters.

Adding a corpus can only tighten the gate, which is what makes the set safe to
grow and safe to ship a thinly-sampled row in. A byte process none of them
describes — minified JS, DNA, a wire protocol — mints its own:
`cargo run --release --example mint -- mine`. [Calibration](#calibration)
overrides all of it; nothing here is hardcoded.

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
  DFAs "a special case of quotient automaton." They measured a multi-fold win,
  and a clear slowdown when nothing rejects — the hazard our cost gate exists
  to avoid.
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
under-prices a long class run by many orders of magnitude, arming patterns into
measured losses), and the accel-aware rival term (the Zig inequality prices the
fronted machine at full-scan cost; this one prices the skip the engine itself
intends to take).

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
