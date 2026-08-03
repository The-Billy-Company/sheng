# Sheng: A Regex Refutation Sieve

A regex can prove a document innocent far cheaper than it can convict one, and that
asymmetry is the whole crate. Sheng builds a register-sized over-approximation of a
pattern's automaton and uses it to prove a haystack match-free before a real engine ever
walks it.

The contract runs one way. A sieve may pass a document that does not match; it may never
reject one that does. Refutation is sound, confirmation is somebody else's job.

Sheng also decides whether to exist. Most patterns should never front a filter, so the
gate prices one against the engine it would sit in front of and declines when it would
not pay - which is the common outcome and the intended one.

## Contents

- [Installation](#installation)
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

It needs Rust 1.95 or newer on edition 2024, and it pulls exactly two crates:
`regex-automata` for the automaton and `memchr` for the narrow byte searches. Both are
already in the graph of anyone using the `regex` crate, so in practice sheng adds nothing
you were not already compiling. It is Apache-2.0.

## Usage

Build the sieve once at startup, then ask it about every document:

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

A `BuildError` is not a failure of your pattern, and every variant means the same thing
to a caller: run your matcher unfiltered. `NotWorthIt` is the usual one, and it carries
the arithmetic that declined it - selectivity, survival rate, and both per-byte prices -
so it prints as a readable explanation rather than a shrug.

A `Sieve` is immutable and holds no scan state, so one instance serves every document
and every thread without cloning.

Hand it the DFA you already built when you want the filter and the confirming search to
be provably the same automaton:

```rust
let sieve = sheng::Sieve::of_dfa(&dfa)?;
```

That also lets the gate read the rival's price off the engine that will actually run,
rather than off a reconstruction of it.

## The Cases That Pay

Sheng pays when one pattern crosses many documents and most of them do not match, which
is the shape of a corpus scan, a log sweep, or a rule slate over a document stream. The
gate confirms or denies that for your pattern in about half a millisecond, so guessing
is unnecessary.

Three properties move a pattern from declined to armed:

- **A distinctive alphabet** - `[0-9]{3}-[0-9]{4}` runs 4.42x, because a quotient built
  over digits and a hyphen refutes almost every position in prose or code.
- **A rival with no cheap accelerator** - `panic!\(` runs 1.470x on arm64, where the
  engine's own `memchr` lead byte is common enough that it cannot skip its way out.
- **Documents in the low kilobytes** - survival rises with length, so a filter that
  clears a 4 KiB document may keep a 64 KiB one on the strength of a single surviving
  position.

Patterns with a rare literal lead byte are the reliable decline, and correctly so.
`regex-automata` crosses a document at 0.0158 ns/B when it can `memchr` its way in,
against the sieve's 0.188, and nothing that inspects every byte fronts a skip twelve
times faster however selective it is.

## The Wrong Tool for the Job

Sheng is a prefilter, so it is the wrong thing to reach for if you want an answer rather
than a refusal. It never tells you where a match is, or whether one exists - only that
one cannot. Use [regex-automata](https://github.com/rust-lang/regex) directly, which is
what this crate calls to confirm every survivor anyway.

It is also the wrong thing for searching a repository. That is `gist`, which is indexed,
ranked, and already tuned for the corpus you are standing in.

## Calibration

The arming decision rests on two measurements that are nobody's universal constants:
what three loops cost on this machine, and what the bytes being searched look like. Both
live in `Policy`, and a caller whose corpus is not source code or whose silicon is not
in `price::MINTED` overrides the field that is wrong.

Adjust the nominal document length when your documents are not the 64 KiB default:

```rust
let mut policy = sheng::Policy::default();
policy.len = 4096.0;
let sieve = sheng::Sieve::with(r"\bTODO\b", &policy);
```

Mint the rest with `cargo run --release --example mint`, pointing `$SHENG_CORPUS` at the
bytes you actually search. The other fields carry the calibration, the byte-generating
chains the fallthrough is judged against, and the marginals the rival's escape set is
priced under.

A machine nobody has measured gets `price::UNMEASURED`, declines every pattern, and says
so through `BuildError::Uncalibrated`. That is fail-closed rather than fussy: the sieve
prices are timed through a byte shuffle, so a target without one would be priced with
pure optimism, and guessing from another machine's silicon is the one option
deliberately not offered.

## Layout

One deep package behind one type, with each module a stage of the pipeline that builds
it. `Sieve` is the surface an ordinary caller needs; the seven stage modules stay
`pub` for two reasons that are not "use them directly" - `Policy` exposes their
measured types (`Calibration`, `Chain`, `Kernel`) so a caller can override them, and
`cargo run --example {mint,bench,skip,survey}` are calibration instruments that read
the pipeline's own intermediate stages rather than duplicating them:

- **`lib.rs`** - `Sieve`, the arming decision, and `Policy`.
- **`projection.rs`** - reachable core states and the exact byte-class partition.
- **`lattice.rs`** - the SP-partition closure, and which quotients to conjoin.
- **`shuffle.rs`** - the register kernel: NEON, SSSE3, and a scalar reference.
- **`skip.rs`** - finding the next byte that leaves the start block, exactly.
- **`selectivity.rs`** - the joint (block, class) chain that predicts `f`.
- **`prior.rs`** - what a byte is likely to be, given the byte before it.
- **`price/`** - what each kernel costs, and the one inequality that decides.

## Development

Run the suite and the four examples from anywhere in the checkout:

```bash
cargo test --release                                   # soundness, judged ungated
cargo run --release --example survey                   # end to end; asserts no armed row loses
cargo run --release --example skip                     # per-lane skip-vs-compose, audits the planner
cargo run --release --example bench                    # per-stage build cost and kernel ns/byte
cargo run --release --example mint                     # re-mint the prior and the calibration
SHENG_NO_SKIP=1 cargo run --release --example survey   # the pre-skip baseline
SHENG_CORPUS=/path/to/your/documents cargo run --release --example mint
```

They read real source, because a synthetic corpus would answer the persistence question
with whatever generator wrote it, and they find it by climbing to the enclosing
checkout. Every emitted constant carries the machine, the kernel and the date that
produced it, from `std` alone with no `uname` subprocess; a measured value with no
machine beside it is an anecdote.

`survey` is a gate, not a report. It asserts that every row the model armed came out
above 1.000x, so a coefficient that drifts generous fails loudly instead of quietly
costing every caller a few percent. `bench` is the opposite - pure report - and it
splits the build into four stages so a regression lands on a named one.

Because it is a gate it also refuses. Each row is timed as five samples of a
min-of-five and carries an interval, and only a row whose whole interval sits below
1.000x counts as a loss; one that straddles is reported undecided and asserts nothing.
Below 8 MiB of corpus it declines to judge at all, because a cache-resident corpus lets
the engine's own accelerator run at tens of gigabytes a second and no per-byte
calibration describes that machine. Aim `$SHENG_CORPUS` at real volume before reading a
verdict into any row.

## The Design

Everything below is how the three interesting parts work: why the approximation is
sound, why the kernel is fast, and why the gate refuses so often.

### Soundness

Partition the DFA's states so the partition is closed under the transition function: if
`p ≡ q` then `δ(p,b) ≡ δ(q,b)` for every byte `b`. That is the _substitution property_,
and the SP partitions of a machine form a lattice under refinement - Hartmanis &
Stearns, _Algebraic Structure Theory of Sequential Machines_ (Prentice-Hall, 1966),
ch. 2.

Every SP partition induces a quotient automaton on the blocks, and marking a block
accepting whenever any member state accepts makes the quotient recognize a superset of
the language:

> the real automaton reaches an accepting state ⟹ every quotient does.

Contrapositive: if a quotient accepts nowhere in the haystack, no match exists. That is
the entire soundness argument, and it is why a survivor proves nothing while a rejection
proves everything.

A conjunction of quotients is sound and strictly stronger than either conjunct, because
the kernel asks for one position where all of them accept rather than for each accepting
somewhere. It is capped at 2, since the marginal quotient costs a full shuffle per byte
and the third rarely earns it. The construction is re-derived and re-checked before it
is trusted, so a partition that is not actually closed is discarded rather than shipped.

### The Parallel Kernel

Langdale's kernel as written has one structural flaw: the register holds the state, so
every shuffle needs the previous shuffle's answer. That is one byte per shuffle latency
forever - 2 cycles a byte on an M4 - and unrolling cannot touch a single dependency
chain as long as the document.

Hold the transition function in the register instead. Seed it with the identity
`[0,1,…,15]` and the same single shuffle per byte composes rather than steps: after any
run of bytes, lane `i` is the block that run would reach had it started in block `i`.
All sixteen answers for the price of one, because the register was already sixteen lanes
wide and fifteen of them were idle.

Nothing in the loop now depends on where the scan is, so the haystack splits into four
slices with no dependency between them and the shuffle unit sees four chains instead of
one. Same instructions per byte; only the critical path shrinks. Geomean over eight
patterns on an M4 is 0.346 to 0.132 ns/byte, a 2.6x gain.

Four slices, because a chain issues one shuffle every `latency` cycles and it takes
about `latency` chains to saturate a port that retires one per cycle. Swept in one
sitting: two slices 0.160, four 0.134, six 0.133, eight 0.140. Two is short of covering
the 2-cycle `tbl`, eight pays more in register pressure and short-document setup than
the extra chains return, and the curve is flat exactly where the latency argument says.

Slicing has an obvious failure mode - a document shorter than one chunk has nothing to
slice - and the fix is not to hand that case back to the scalar walk. A 64-byte document
is entirely final chunk, and doing the obvious thing made small documents several times
slower than the kernel they were meant to be using. The short tail re-derives its own
stride instead, and every size gets faster:

- **64 B** - 0.290 ns/byte before, 0.238 after, a 1.22x gain.
- **256 B** - 0.401 before, 0.190 after, 2.12x.
- **1 KiB** - 0.477 before, 0.186 after, 2.56x.
- **4 KiB** - 0.479 before, 0.181 after, 2.65x.
- **64 KiB** - 0.388 before, 0.131 after, 2.97x.

The accept test survives intact. Composition gives a per-lane running max, so `high[i]`
is the highest block the slice would have visited from `i`, and collapsing the slices
reads each slice's max at exactly the lane the real trajectory entered it on. That is
the true maximum over the chunk rather than a bound on it, so the parallel kernel
refutes exactly the documents the scalar reference refutes.

Reading all sixteen lanes would also be sound, since over-reporting an accept costs a
skip and never a wrong answer. But it would treat sixteen hypothetical scans as if they
had all happened, and on an unanchored pattern almost nothing would refute.

Deleting the max instead of resolving it is possible, and measured not to be worth it.
Give every accepting block a self-loop and "did it ever accept" collapses into the final
state, dropping per-byte work from load+shuffle+max to load+shuffle - which benchmarks
at 0.131 ns/byte against 0.134, inside the noise, because the loop is bound by the row
load and the shuffle port. Buying two percent would have cost a second trapping form of
every quotient, since the selectivity chain needs the honest one. Prototyped, measured,
reverted.

### The Start-Block Skip

At 0.131 ns/byte the loop is load-port bound at two loads per byte, the row and the
haystack, so there is no instruction left to shave. The only remaining move is to stop
reading bytes.

Which is available, because most bytes teach the sieve nothing. A quotient sits in its
start block until a byte _escapes_ it, and while it sits there every self-loop byte is
provably a no-op: same block before, same block after. Unanchored patterns live there
almost entirely - 98.2% of corpus bytes for `WalletService`, 99.2% for
`#[0-9a-fA-F]{6}` - so `skip::Skip` finds the next escape directly and the kernel jumps
to it.

Two instruments, chosen by how many byte values leave the block:

- **1-3 values, `memchr`** - already in the graph transitively via `regex-automata`, and
  the best-tuned SIMD byte search in the ecosystem. Writing a fourth-best one to avoid a
  direct edge would be vanity.
- **4-128 ASCII values, a nibble classifier** - `lo[b & 0xF]` carries one bit per high
  nibble, `hi[b >> 4]` selects it, and the product is nonzero exactly for members.

The classifier is not ours. It is Wojciech Muła's
[SIMDized check which bytes are in a set](http://0x80.pl/articles/simd-byte-lookup.html)
(2018) - the set as a 16x16 bitmap addressed by nibbles, tested with the same `pshufb`
the kernel already leans on - and it ships in Hyperscan as `shufti`.

What we took from the article is its constraint, not just its trick. Muła needs two
bitmap pairs to cover all 256 values, because `pshufb` zeros a lane whose index has the
high bit set, and he recovers the 8..15 half with a second lookup on `indices ^ msb`.
Sheng carries one pair and therefore covers `0x00..=0x7F` only.

That makes the classifier exact rather than a prefilter over the range it accepts, and
it refuses outright any set holding a byte at or above `0x80` or wider than 128 values:
`Skip::of` returns `None` and the lane falls back to composition. Approximating instead
would be the one unrecoverable bug in this crate, because a classifier that quietly
answers "not a member" for a byte that really does escape does not run slow - it skips
past a real transition, and a sound refutation becomes a missed match.

The choice is per lane, and it has to be, because skipping frequently loses. Against a
one-byte escape set it runs 8.8-11x the composition kernel; against a three-way
alternation it runs 0.25x, because a scalar excursion loses badly to four lanes
advancing at once. So `Lane::plan` prices both and takes the cheaper, and a sieve with
two conjuncts can legitimately run one of each. On the survey slate the planner matches
the measured winner 8 times out of 8, declining the skip in precisely the four cases
where it would have cost 1.4-4x.

Pricing it needed one new coefficient, because reusing the engine's `dfa_excursion`
badly under-predicted the shufti path - the sieve resumes a register shuffle where the
engine re-enters a full DFA - so `skip_excursion` is minted per instrument. Getting it
stable also forced every ratio to be timed interleaved with its own baselines, one
traversal of each per round. This laptop runs ten coworker agents at load average 12,
contention does not fall equally on a branchy excursion and a streaming `memchr`, and
the same sweep that read 5.33 and 9.08 on consecutive unpaired runs holds still when
paired.

The classifier is hand-written NEON and SSSE3, so it is differentiated against a scalar
statement of the same set membership, three ways:

- **Every byte value against every set shape** - all 256 values for each set, so a
  single misclassified byte fails rather than needing a haystack that contains it.
- **2048 pseudo-random ASCII sets** - from a seeded xorshift sweeping widths 1-96,
  because nine hand-picked sets are nine sets we thought of and nibble aliasing is
  exactly the bug intuition steers around.
- **A planted escape at every offset of every length from 1 to 72** - the one that
  catches a vector search dropping its remainder. That bug does not crash; it returns
  `None`, the sieve skips a real escape, and a match goes missing.

The end-to-end suite then asserts a skip lane was actually chosen before comparing
`refutes` against `refutes_scalar`, and draws half its haystacks from a narrow alphabet.
Uniformly random bytes leave the start block within a byte or two, so a skip loop over
them never takes a long jump and never reaches its tail - the differential would have
run, passed, and tested nothing.

### The Cost Gate

We built the filter first and it made everything slower: geomean 0.230x, a 4.3x
regression across a thirteen-pattern slate. The kernel was genuinely fast; the problem
was that it armed on every pattern that could harvest a quotient. Two lessons came out
of that, and neither is obvious from the outside.

Position rejection is not document rejection. A filter that retires 99% of byte
positions sounds decisive and is not, because one survivor drags the entire buffer into
verification - over a 4 KiB document, `1 - (1-f)^4096` at `f = 0.01` is 99.99% survival.
The quantity that matters is documents kept, and it rises vastly faster than the
per-position rate falls.

`[0-9]{4}-[0-9]{2}-[0-9]{2}` rejects 99.03% of positions and keeps 80% of documents,
because dates cluster.

The rival's price then decides more often than the filter's selectivity does, and a gate
phrased as a threshold on selectivity has no term in which to see that. So the gate is
an inequality between two measured costs:

```text
sieve  +  (1 - (1-f)^len) * rival   <   rival
```

Every term is absolute nanoseconds per byte, and the rival's term is not assumed: it is
read from `Automaton::accelerator` on the engine's own start state, so the crate asks
the engine which bytes it intends to skip and prices it accordingly.

### The Persistence-Aware Prior

Predicting `f` without a calibration haystack is the interesting half, because a filter
that needs a sample of the document it is about to filter has already paid for the scan.
The estimate comes from the quotient's own Markov chain and nothing else.

The obvious chain runs on the quotient's sixteen blocks and draws each byte
independently, which prices a `k`-byte class run as `p^k`. Real text is not memoryless -
classes cluster ferociously. Measured over 64 MiB of this repository:

- **`Lower`** - marginal 0.5703, repeat probability 0.7683, a 1.3x ratio.
- **`Punct`** - marginal 0.1325, repeat 0.2524, 1.9x.
- **`Space`** - marginal 0.1817, repeat 0.4517, 2.5x.
- **`Upper`** - marginal 0.0560, repeat 0.3565, 6.4x.
- **`Digit`** - marginal 0.0186, repeat 0.3863, 20.8x.
- **`High`** - marginal 0.0139, repeat 0.9167, 66.0x.

A digit is twenty-one times likelier to follow a digit than to occur at random, so an
independent-draw model under-prices a forty-digit run by `20.8^40`. That is not a
rounding error; that is a filter believing it rejects everything while rejecting
nothing.

So the chain carries the byte's class in its state and runs on (block, class) pairs -
112 states, a 7x7 transition matrix, a uniform spread within each class. Under a
memoryless prior it collapses back to the naive chain exactly, which makes it a strict
generalization rather than a rival model. `Prior::Text` is kept precisely because it is
the superseded one: a model you can still address is a model whose error you can still
measure.

That chain was also the entire build cost. Solving it took 512 power iterations over 112
states, each re-reading all 256 bytes for every block, and that one function was 99.9%
of building a sieve - 44 to 75 ms of a 44-to-75 ms build. A filter that takes 60 ms to
decide whether it can save you 60 ms is a filter nobody arms.

It factors. The byte draw depends only on the previous class and the destination block
only on `(block, class)`, so aggregating the 256 bytes into class edges once at
construction lets the inner loop visit the transitions that exist rather than the 256
that might. Same arithmetic, same predictions, no approximation introduced. Measured on
an M4 against `HEAD` with the same harness and corpus, as selectivity before and after,
then whole build before and after:

- **`WalletService`** - 63.0 ms to 0.40 ms; build 64.1 ms to 0.55 ms.
- **`(alpha|beta|gamma)`** - 66.3 ms to 0.42 ms; build 64.6 ms to 0.54 ms.
- **`[0-9]{3}-[0-9]{4}`** - 44.4 ms to 0.28 ms; build 44.4 ms to 0.31 ms.
- **`<[^>]*>`** - 52.4 ms to 0.16 ms; build 52.1 ms to 0.19 ms.
- **`#[0-9a-fA-F]{6}`** - 74.5 ms to 0.29 ms; build 72.6 ms to 0.34 ms.

That is 116x to 278x on the whole build, and the projection and harvest stages that were
rounding errors before are now most of what is left, at 20-32 µs and 4-79 µs. Both
columns were taken back to back on the same idle laptop against the same corpus, which
is the only way they are comparable - absolute figures drift 25% under load, so read the
ratio and not the digits.

One thing did not survive: exiting the iteration early once the distribution looks
settled. The quantity being estimated is the accepting mass, which can be `1e-29` and is
still climbing long after the bulk of the distribution has stopped moving, so a
settled-looking chain returned a clean `0.0` where the byte-level reference said
`1.2e-29`. The iteration count is fixed, and the loop is now cheap enough that it does
not matter.

### The Excursion Coefficient

`dfa_excursion` is the one term we could not derive. An accelerated engine does not pay
for a single byte when `memchr` trips - it enters the DFA, walks a run, returns, and
restarts the skip - and the restart dominates at that granularity. Omitting it
under-priced a common-byte accelerator by 8x, which declined patterns that genuinely
paid.

So we solved it rather than picking it: time eleven accelerated patterns whose lead
bytes span two orders of magnitude of frequency, then invert the blend for `E`. The
first attempt read escape frequency at class resolution and the eleven answers spanned
3.6 to 35.2. A tenfold disagreement is a model telling you it is wrong.

Reading the same eleven from a per-byte table collapses them to roughly 7.5 to 13, mean
~10.3 and stable to a percent across re-mints, and that collapse is the evidence the
coefficient is real. The top of the band still wanders run to run, because its lead byte
occurs in one position in eight hundred and there is only so much signal in that.

The slate classifies perfectly: six armed rows, seven declined, every declined row
independently measured below 1.0x when forced to arm, and no armed losses run after run.

Read the geomean over armed rows carefully, because it is the one number here that can
mislead. It reads 2.21x with the skip kernel on and 2.83x with it off, under
`SHENG_NO_SKIP=1`, and the skip is nonetheless the improvement: it arms two patterns
that previously could not pay for themselves at all, both marginal (`WalletService`
1.16x, `foo[^\n]*bar` 1.09x), so the set being averaged grew by two low rows. Held to
the four patterns that armed either way, the same slate goes 2.83x to 3.11x, nearly all
of it `[0-9]{3}-[0-9]{4}` moving 3.21x to 4.42x.

### Machine Dependence

Three layers of this crate could in principle be tied to the machine that measured it,
and only the last is a real limit.

The mathematics and the kernel are not tied to anything. The SP-quotient closure is
arithmetic over an automaton; the kernel has NEON, SSSE3 and a scalar path, and the
crate reports which it picked through `shuffle::kernel()` so the differential test
cannot pass vacuously. Both vector paths are exercised - `Neon` natively on arm64,
`Ssse3` on x86_64 first under Rosetta and then natively on a Debian box - each agreeing
byte-for-byte with the scalar reference across 30 000 mutated haystacks.

The clock is provably irrelevant. Multiply every coefficient of a `Calibration` by any
positive constant and no decision moves, because the factor cancels on both sides of the
arming inequality. That is a test named
`scaling_the_whole_calibration_changes_no_decision`, not a hope, so a loaded laptop, a
thermal-throttled core, and a 3 GHz-versus-5 GHz difference all decide identically.

What survives the scaling is three dimensionless ratios. Both rows in `price::MINTED`
are minted from this crate's own real source tree (`examples/common.rs::root` climbs to
the nearest `.git`, so any clone re-derives them the same way) - there is no external
dataset to acquire before `cargo run --release --example mint` reproduces either row:

- **`skip/walk`** - 0.013 on the arm64 laptop, 0.010 on the x86_64 box.
- **`sieve/walk`** - 0.149 on arm64, 0.175 on x86_64.
- **Excursion** - 9.7 on arm64, 11.6 on x86_64.

Those three are a real property of an instruction set, and they differ, but less
sharply than instinct suggests once both sides are timed with the *same* kernel.
Absolute walk cost is nearly identical at 1.26 against 1.25 ns/B, a dependent-load
chain either way, and on the ratio that decides most rows the two boxes now land within
a quarter of each other rather than the twofold split an earlier, kernel-mismatched
pairing showed. Both rows were re-minted together on 2026-08-03, same day and same
procedure, after `shuffle` was rewritten to compose four slices in parallel instead of
holding the state in one register - the prior x86_64 row had been timed
against the serial kernel and said so in `price/minted.rs` rather than quietly carrying
that mismatch as current. With the mismatch gone, `panic!\(` now arms on both: 1.329x on
arm64, 1.319x on x86_64.

`price::MINTED` holds one row per (architecture, kernel) pair anybody has measured and
`price::active()` resolves the running machine against it. The refusal for everything
else was confirmed on the box before its row existed - thirteen patterns, thirteen
declines naming the missing measurement.

The corpus is the other honest limit. The shipped priors describe a polyglot source
tree, and English prose, JSON logs, or minified JavaScript are different byte processes.
None of that is hardcoded, which is what [Calibration](#calibration) is for.

## Prior Art

This crate is a Rust port of one rung of our Zig engine
[irregex](https://github.com/The-Billy-Company/irregex), not a second design. Both
halves come across - the SP-quotient harvest that builds the over-approximation, and the
Sheng-shaped register kernel that runs it, which in the Zig tree are `quotient.zig` and
`sheng.zig` inside the same `sieve/` package. The codename is the kernel's, because the
kernel is what makes the idea affordable.

The contract - over-approximate, reject early, verify survivors exactly - is not novel,
and it would be dishonest to imply it. It is prior art at least three times over:

- **Luchaup, De Carli, Jha & Bach** - _Deep packet inspection with DFA-trees and
  parametrized language overapproximation_, INFOCOM 2014
  ([10.1109/INFOCOM.2014.6847977](https://doi.org/10.1109/INFOCOM.2014.6847977)). Their
  Definition 7 is exactly `|D'| < |D|` with `L(D) ⊆ L(D')`, matching stops at the first
  rejecting node, and the paper calls its shrunk DFAs "a special case of quotient
  automaton". Measured 4.7x - this is the same idea. They also measured +26% when
  nothing gets rejected, which is the hazard our whole cost gate exists to avoid.
- **Češka, Havlena, Holík, Lengál & Vojnar** - _Approximate reduction of finite automata
  for high-speed network intrusion detection_
  ([arXiv:1904.10786](https://arxiv.org/abs/1904.10786), 2019), a cascade of small crude
  over-approximating NFAs, the approximation chosen by a probabilistic model of the
  traffic.
- **Hyperscan's `HS_FLAG_PREFILTER`** - shipping for years: matches are a superset and
  the caller confirms with an exact matcher.

The kernel is not ours either. It is Langdale's Sheng (2018,
[branchfree.org](https://branchfree.org/2018/05/25/say-hello-to-my-little-friend-sheng-a-small-but-fast-deterministic-finite-automaton/),
shipped in Hyperscan) - a DFA of ≤16 states held in one vector register and stepped with
a single `pshufb` / `vqtbl1q_u8`, no gather and no memory for state. What we did was
point it at an over-approximating quotient instead of at the real automaton, which is
what lets a machine that must fit in a register front a pattern far too large to fit in
one.

Making that kernel parallel is not ours either. It is the enumerative approach of
Mytkowicz, Musuvathi & Schulte, _Data-parallel finite-state machines_, ASPLOS 2014
([10.1145/2541940.2541988](https://doi.org/10.1145/2541940.2541988)) - break the
dependence by computing transitions from all possible states at once, note that this is
a gather, and implement the gather with `_mm_shuffle_epi8` on machines that lack one.
They say the quiet part out loud too: "ILP because gather is associative", which is
function composition by another name.

It costs nothing here for a reason worth naming, since their paper's central difficulty
is that enumeration's overhead grows with the number of states - enough that they need
two convergence optimizations to claw it back. A sieve has already capped its machine at
sixteen blocks to fit the register at all. The over-approximation that makes the filter
sound is the same thing that makes enumeration free, and those two facts are not related
by design; they just happen to be the same constant.

So the narrow claims are three. The SP-lattice harvest as the source of the
approximation, rather than a hand-built tree of shrunk DFAs or a model learned from
traffic; the ≤16-state register-resident conjunction selection that keeps the filter at
one shuffle per byte; and the training-free gate, with no observed traffic, no learning,
and no runtime self-disable.

Two pieces of that gate are not ports either, and both were named as open residuals by
the Zig rung's own bench rather than discovered fresh:

- **The persistence-aware prior** - the Zig gate estimates selectivity under a
  memoryless byte prior, which under-prices a 40-byte run by seventeen orders of
  magnitude and armed one pattern into a measured 0.89x loss.
- **The accel-aware rival term** - the Zig inequality prices the machine it fronts at
  full-scan cost, where this one asks the engine which bytes it intends to skip and
  prices the skip.

## Problem Reports

A wrong answer is the report this crate most wants, and the bar is low: if `refutes`
returned `true` for a document that matches, that is a soundness bug and it outranks
everything else here. Include the pattern, the bytes, and the output of
`shuffle::kernel()`.

Route everything else by which surface you actually used. Bugs and vulnerabilities in
this crate go through [SECURITY.md](SECURITY.md) and
[CONTRIBUTING.md](CONTRIBUTING.md).

Two neighbors own surfaces that are easy to mistake for this one. Anything about what a
pattern means - parsing, Unicode, match semantics - belongs to
[regex-automata](https://github.com/rust-lang/regex), because this crate reads that
crate's `dense::DFA` and never parses regex itself. Anything about the Zig original,
including the `sieve/` package this was ported from, belongs to
[irregex](https://github.com/The-Billy-Company/irregex).

## Non-Negotiables

Soundness is a property of the construction, so the differential harness tests every
pattern that harvests a quotient, not the minority the economics admit. Testing only the
armed ones would shrink the suite every time the gate got stricter, which is exactly
backwards: a false reject is a missed match, and that risk exists the moment a quotient
exists. `Gate::Ungated` is there for that reason and no other.

The gate declines most patterns, and that is the design working. A prefilter's honesty
is measured by how often it refuses.
