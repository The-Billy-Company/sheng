# Changelog

<!-- towncrier release notes start -->

## [0.2.0] - 2026-08-10

### Added

- A `Dfa` trait, and with it a `no_std` build: `--no-default-features` is now a crate
  whose only dependency is `memchr` and whose only requirement is an allocator.

  This was never a port so much as a boundary finally drawn where the code already
  divided. Nothing in `regex-automata` was ever on the scan path — it parses a pattern
  and hands over a `dense::DFA`, and after that the sieve is arithmetic over sixteen
  bytes of table. The projection asked exactly six questions of that automaton, so those
  six are now the `Dfa` trait, `regex-automata`'s dense DFA is one implementor of it, and
  `Sieve::of_dfa` is generic over the rest. A hand-written transition table, a
  zero-copy deserialized automaton, or an engine behind an FFI boundary can all drive
  the whole pipeline now, and `tests/dfa.rs` proves it by doing so from four states and
  three byte classes with no engine anywhere in the file.

  That also closes a semver leak nobody had tripped on yet: `Sieve::of_dfa` and
  `Projection::of` used to name `regex_automata::dfa::dense::DFA<Vec<u32>>` in their
  signatures, which made a major version of somebody else's crate a major version of
  this one.

  Getting to `no_std` then cost less than expected, because what it removed was mostly
  `std` being asked for things `core` already knew:

  - `f64::powf`, the crate's only transcendental, was the survival term's `(1-f)^len`.
    The exponent is a **count of bytes**, so exponentiation by squaring is the whole
    operation — held to `powf` across nine lengths and nine rates in a test that runs
    wherever there is a `std` to disagree with. Every float operation in the crate is now
    `+ - * /` and a comparison. No `libm`, no math library behind it.
  - `std::env::consts::ARCH` is a per-target compile-time constant that only looked like
    it needed an operating system to ask. `price::ARCH` reads it from `cfg` and is
    checked equal to `std`'s.
  - `std::arch::is_x86_feature_detected!("ssse3")` is now one `CPUID` leaf read, memoized
    in an `AtomicU8`. This is the case where hand-rolling a feature probe is *equal* to
    `std`'s rather than weaker than it: SSSE3 is a plain `CPUID.01H:ECX[9]` bit with no
    operating-system participation, unlike the AVX-512 family where a set bit only means
    the silicon can and `XGETBV` has to be asked whether the kernel will. One
    implementation for both configurations, not a `cfg` with two answers.
  - Two `HashMap`s in the projection became a sorted `Vec` and one reused scratch column.
    This was supposed to be a lateral move to drop a `std` type and to ask only `Ord` of a
    caller's state type instead of `Hash` plus a hasher. It is **2.5-3x faster**
    (`cargo run --release --example bench`, `project µs` over the eight-pattern slate,
    three runs each): 32→13, 31→12, 27→10, 24→8.5, 21→7.7, 14.4→4.1, 22.8→8.0, 23.1→7.9.
    The reachable core is capped at 96 states, where seven compares beat hashing an opaque
    key — and the class refinement had been allocating 256 vectors and hashing up to 192
    bytes per byte value to discover that most of them were duplicates.

  `memchr` stays, unconditionally and deliberately. It is `no_std`-capable already, it is
  on the scan path for 1-to-3-byte escape sets, and nothing hand-rolled here would match
  its tuning at that width — so the `std` feature now just elects its AVX2 runtime
  dispatch instead of the crate insisting on it.

  Two things fell out of building for `x86_64-unknown-none`, which is soft-float and has
  no SSE at all: the SSSE3 kernels are now gated on `target_feature = "sse2"` rather than
  on the architecture alone (without it a 128-bit vector cannot be held, and the code
  generator says so), and such a target honestly reports `Kernel::Scalar`. CI checks the
  complete powerset of the two features, both bare-metal targets, and runs
  `tests/dfa.rs` against the library with its dependencies and its `std` removed — because
  compiling is not running.
- A caller who declares documents shorter than the calibration was measured over now gets
  `BuildError::Unmodeled` instead of a speedup figure, governed by the new
  `price::VALIDITY_FLOOR`.

  The gate charges every loop in ns/byte, so a document's price is linear in its length and
  passes through the origin. Two things it therefore cannot see grow as the document
  shrinks. Both loops pay a per-*call* cost — table loads, a masked tail, a restarted
  `memchr` — worth under a percent of the sieve at `NOMINAL_LEN` but around half of it at 64
  bytes. And the sieve's edge over a walking rival, flat from a few kilobytes up, collapses
  to roughly a third of that by 64 bytes. Both inflate the predicted speedup, so both argue
  for arming, and near the threshold a caller would have been armed on the difference.

  `MARGIN` decides where that stops being imprecision and becomes the whole verdict, and the
  crossing is measured — see *A slate measured rather than asserted*, which swept it: the
  edge holds within a couple of percent down to a kilobyte, is 16% under nominal at 256
  bytes, and 39% under at 128. So the existing cushion absorbs every length at or above the
  floor and none below it, the floor sits there rather than at a rounder number, and under it
  the answer is not "your
  pattern does not pay" but "this row was not measured over documents like yours" — which is
  why it is its own variant and not a `NotWorthIt`. `Gate::Ungated` consults no price and
  ignores the floor, so a caller who knows their own traffic can still have the filter; what
  is no longer on offer is a promise with no measurement behind it.

  Measured rather than assumed, and the same sweep retired a coefficient before it was built:
  a per-document fixed cost was the leading candidate for the missing term, and adding it
  would not have moved the floor. It is real on the sieve's side and a few nanoseconds
  across, but the larger short-record effect belongs to the rival and runs the other way, so
  there is no constant to mint that makes the ratio right. `NOMINAL_LEN` records that
  negative result, and why 4096 is the knee of the measured ratio rather than only a margin
  against an optimistic fallthrough estimate.
- A counted repeat no longer refuses a pattern before anything has been priced. `Policy::relax`
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
- A machine with no row in `price::MINTED` can now measure its own. `Calibration::measure`
  takes a sample of the caller's documents and hands back a row; `price::Bench` is the same
  measurement with the knobs, and `Bench::report` adds every solution behind every solved
  coefficient. The row goes into `Policy::calibration` and the gate starts deciding.

  `riscv64`, `powerpc64`, `s390x` and `loongarch64` were **inert**, and the reason was worth
  naming precisely because it was not portability: every kernel in this crate compiles and
  runs there through `Kernel::Scalar`, which needs no vector instruction at all. What those
  machines lacked was five coefficients, and the only way to get them was to clone this
  repository, run an example, paste a `const`, open a pull request and wait for a release. So
  the whole audience on unminted silicon was refused for want of a measurement their own
  machine was standing there able to take.

  A runtime row is *better* evidence than a shipped one on two counts. It describes this
  machine rather than one sharing its `(os, arch, kernel)` triple — which cannot tell an
  M-series laptop from a datacenter Ampere — and it is taken over the caller's own bytes, so
  the marginals every escape set is priced under are measured rather than borrowed from one of
  four shipped corpora. What it gives up is reproducibility, and the mitigation is the mint's:
  a minimum over several traversals, with every ratio's legs interleaved so contention falls
  on numerator and denominator alike. `price::histogram` is public for the same reason, since a
  caller measuring their own corpus wants `Policy::freq` to describe it too.

  Every refusal in `Unmeasurable` is a case where a row could have been returned and would have
  been fiction. A sample under `MEASURABLE_ABOVE` is refused because at a few kilobytes the
  clock's granularity *is* the measurement; a sample holding the `\x00\x01zz` sentinel is
  refused because the reference patterns would match and time an early exit rather than a
  traversal. One sample is one memory regime, so a row fills the column its own byte count
  earns and leaves the other reading unmeasured — `Calibration::regime` names which, and
  `Calibration::merge` combines a cache-sized sweep with a memory-sized one, refusing a pair
  whose cache column prices the engine's skip *above* its memory column, since a hotter
  haystack cannot cost more and such a pair measured a busy machine.

  `examples/mint.rs` now calls all of this instead of carrying its own copy, and is what it
  always should have been: the *publishing* half. It still prints the pasteable `const`, the
  two fields a runtime row cannot fill (`host` and `minted` are `&'static str`), the per-kernel
  sweep through `shuffle::force`, and the spread beside every mean. Three hundred lines of
  timing loops, reference patterns and inversions left it. The duplication was a live drift
  hazard rather than untidiness: the slate a `sieve` coefficient is timed over and the slate a
  skip is judged against are supposed to be the same slate, and with two copies nothing said
  so. `BuildError::Uncalibrated` now names the remedy, since it is the only variant in that
  enum a caller can lift from where they stand.
- An AVX2 kernel, and a dispatch rule that will not run a kernel nobody measured.

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
- The gate can now be told how many searches one refutation lets a caller skip.
  `Policy::rivals` is one by default — one pattern, one engine, one document, which is the
  shape the arithmetic was written for — and above one it makes a **slate** a different
  economic proposition from a pattern rather than the same one repeated. The pre-pass is paid
  once per document while verification is paid once per rival, so the inequality divides
  through to `(sieve/rivals + survival * rival) * (1 + MARGIN) < rival` and the sieve's own
  price stops being what declines a near-parity filter.

  That term was the difference between the workload this crate is theoretically best at and
  the one it could actually serve. A secret scanner, a log triage rule set, a classifier — all
  of them run tens of patterns over each document, and every one of them was being priced as
  though a refutation saved a single search. `src/price/gate.rs` carries the unit tests for
  what the term may and may not do: it is monotone in `rivals`, it amortizes the pre-pass and
  *only* the pre-pass, and it converges on `survival * (1 + MARGIN) < 1` — so a filter that
  retires nothing is unrescuable at any fan-out, which is the same ceiling `Rival` converges
  on for the same reason. `tests/slate.rs` finds the crossing on a real pattern pair and also
  keeps an unrescuable one, so the limit is exercised rather than described.

  Two obligations come with it and neither is checkable from inside the crate, so both are
  stated on the field. One refutation really must skip them all, which means the sieve has to
  come from an automaton whose language contains every pattern's — `Sieve::of_superset_with`
  is the direct way, and a union automaton the direct source. And the priced rival should be
  the *cheapest* of the slate, since the engines in a real slate differ by an order of
  magnitude and underestimating the rival can only make the sieve decline.

  `BuildError::NotWorthIt` names the fan-out in its message when there is one, and says
  nothing when there is not — so a single-pattern decline reads exactly as it did, while a
  caller who declared a slate and still declined can see the term was applied and was not
  enough rather than wondering whether it was read at all.
- The gate can now be told that a survivor costs something other than a regex scan.
  `Policy::rival` takes a `Rival`, which is `Rival::Engine` by default — read the price off
  the automaton, exactly as before — or `Rival::Walks`, a dimensionless multiple of this
  machine's dense-DFA walk, or `Rival::NanosPerByte` for a confirm somebody has actually
  timed. `Walks` is the one to prefer, because a ratio rescales with the row it is a ratio
  of and so `scaling_the_whole_calibration_changes_no_decision` keeps holding; a duration
  does not, which is asserted rather than admitted so the caveat stays measurable.

  This was the crate's largest unreachable audience. A refutation's product is a proof that
  a document needs no further work, and what that is worth is set by what the work would
  have been — but the price could only be read off a `Dfa`, which describes what the
  *pattern* costs to confirm and not what the caller's pipeline costs to run. A caller could
  only forge a `Calibration` and misuse `dfa_walk` to mean something it does not, corrupting
  `skip_per_byte` in the same motion. `examples/census.rs` now sweeps the same 31-pattern
  population against a document extraction as well as against the engine.

  A costly rival is nonetheless inert on its own, and `Policy::bypass` is the term that says
  why — see *A baseline the caller would really have run*. An expensive rival divides the
  pre-pass and cannot manufacture selectivity either: as the price grows the gate converges
  on `survival * (1 + MARGIN) < 1`, the same limit `Policy::rivals` converges on for the same
  reason, so a filter that retires nothing is unrescuable at any price and `tests/rival.rs`
  holds it there. The documentation also names the confirms that do *not* qualify, which is
  the more useful half: a walk is 1.3–2.1 ns/byte on the minted machines, so gzip, AES with
  hardware support, and JSON parsing are all within a small multiple of the engine and
  change nothing.

  `Residency::of_working_set` turns a byte count into a regime against the new
  `price::RESIDENT_ABOVE`. The residency question stays the caller's, because this crate
  still cannot see the corpus — but a caller who knows how many bytes they are about to hand
  the engine was being asked to answer it twice, and `examples/survey.rs` had the arithmetic
  copied into it. The one way it can be wrong is named on the function: a re-scanned working
  set is cache-resident however large, and that error arms rather than declines.
- The gate no longer prices a sieve against doing nothing. `Policy::bypass` takes a `Bypass`
  — `Engines` by default, or `Slate(Rival)` for a caller who can name the whole alternative,
  or `Absent` for one who has none — and `CostFact::unfiltered` is now the **cheaper** of the
  rival slate and that bypass.

  This closes the crate's largest arithmetic hole, and it is worth stating as the objection
  it answers. `Rival::Walks(512.0)` armed 24 of 31 census patterns, and that number was a
  comparison against a pipeline nobody runs. If a survivor costs 512 walks a byte then a walk
  costs 0.2% of a survivor, so a caller holding a regex would put the engine in front of the
  extraction — an *exact* filter, whose survival rate is the true hit rate rather than the
  sieve's fallthrough. At a secret scanner's ~1e-4 that alternative costs about 1.05 walks a
  byte against the sieve's ~410, and it wins by two orders of magnitude. The gate compared
  the sieve against no filter and never against the cheap exact one already in the dependency
  graph, so the regime where most patterns armed was the regime where arming was mostly
  wrong.

  Measured with the term in place, `examples/census.rs` now reports 11 of 31 arming in front
  of the engine and **the same 11** in front of the 512-walk confirm, because the engine is
  still there to run first. Only `Bypass::Absent` reaches 24, and that is the honest value of
  a costly rival: it belongs to a caller who genuinely cannot decide the question more
  cheaply where the sieve runs — screening packets against rules whose matches only exist in
  a reassembled flow, not "my confirm is slow". `tests/rival.rs` asserts all four corners,
  including that a nonsense price still cannot arm anything through the new term.

  `CostFact::ceiling` is the companion, and `BuildError::NotWorthIt` now prints it. Every
  amortizing term converges on `1 / survival`, so the ceiling is the most any rival price or
  slate size could ever have reached, and a decline under the margin at its own ceiling is
  not arguable. Six of the seven census declines are terminal in exactly that sense — which
  the summary now separates rather than reporting one number for two findings.
- Two more kernels, a list that will not let one sleep quietly, and a mint that can finish.

  `src/arch/` gains AVX-512 for x86_64 and SIMD128 for `wasm32`, which puts the ladder at
  four rungs on x86_64 and two on WebAssembly. Both are the same two kernels the other
  backends implement — the composition sweep and the skip classifier — and both agree with
  `shuffle::scalar` byte for byte, which is the only thing that could make either correct.

  AVX-512 is 64 bytes per `vpshufb`, and for this crate that is *four* slices per register
  rather than one wider slice, for the reason AVX2 gets two: `vpshufb` is defined per
  128-bit lane at every width it exists at, and the sieve's step is a 16-entry table lookup,
  so a `zmm` carries four independent trajectories rather than one four-times-wider one.
  `vpermb` is deliberately declined — a cross-lane permute buys a wider table, and the table
  is `LANES` entries because that is a quotient's block count and not because sixteen is what
  a register held. The classifier does get something the narrower kernels cannot have:
  `vptestmb` writes "these lanes ANDed nonzero" straight into a mask register, which is
  exactly the membership question, where SSSE3 and NEON must compare against zero and invert.

  The probe grew with it. AVX-512 needs `avx512f` and `avx512bw` from `CPUID` and three
  `XCR0` bits — opmask, and both halves of the upper `zmm` state — because a width whose
  registers the operating system will not preserve across a context switch is not a width the
  process has, and `CPUID` alone cannot say. Same shape as the `ymm` check AVX2 already
  carried, one register wider.

  AVX-512 also gets a CI leg of its own, under Intel SDE, because it is the one rung whose
  correctness no runner here can be relied on to prove — the native legs sweep whatever the
  probe admitted, and which x86_64 parts the fleet allocates is not something a workflow
  decides. The leg fails loudly if `arch::available` does not come back holding `Avx512`,
  since a leg that quietly re-tested SSSE3 under an emulator would be worse than no leg.
  Emulation is allowed to prove a kernel correct and is never allowed to price one:
  `price::MINTED` takes rows from real silicon and nowhere else.

  `wasm32` is the first target whose kernel is chosen at compile time rather than probed: a
  guest has no `CPUID`, so `-C target-feature=+simd128` decides, and `arch::available` reads
  `cfg` where it reads a register everywhere else. A new CI leg builds both halves and runs
  the differential under `wasmtime`, handing the corpus in through a WASI preopen — the
  SIMD128 kernel had compiled on nothing and executed nowhere before that leg existed.

  `price::DORMANT` is the new list, and it is the answer to a failure this crate had already
  built the machinery to avoid and no way to notice. Dispatch declines a kernel `MINTED` has
  no row for, which makes shipping an unmeasured kernel safe — but a kernel nobody has minted
  yet and a kernel that *stopped* being priced are indistinguishable in `MINTED`: both are
  simply absent. The first is a plan and the second costs every machine on that silicon its
  throughput while every test still passes, because the narrower kernel is still correct.
  `DORMANT` names the first with its reason, and a test holds the two lists to each other in
  both directions — so a kernel left out fails the build, and a row landed later fails the
  build until its line here is deleted.

  Which leaves the mint, whose x86_64 leg is what all three of those kernels are waiting on,
  and which could not have completed if anyone had dispatched it. A price row is nanoseconds
  per byte read from *memory*, so `examples/mint.rs` refuses a corpus under 32 MiB — and the
  workflow's corpus was ~14 MiB, sized for `survey`'s 8 MiB prior floor. Worse, the refusal
  sat after the persistence sweep and the workflow's own size gate sat after the whole run, so
  the failure mode was most of an hour of a runner's time to report a precondition. Four
  pinned upstream standard libraries (Zig, TypeScript, swift-syntax, and two of Go's
  packages) bring the corpus to ~60 MiB across five language families, and `mint -- corpus`
  applies the same floor by walking the same tree with the same code, in seconds, before the
  measurement rather than during it.

  `native.yml` is now called from `ci.yml` as well as `release.yml`, which is what its own
  header has always said and what `README.md` has always claimed: the six-cell cross-OS proof
  on real, never-emulated silicon ran on tags only. And no CI leg is named for a kernel any
  more. `arch::available` probes at runtime, so which rungs an `x86_64` runner offers is a
  fact about the machine the fleet allocated — a leg labelled `x86_64-ssse3` was a guess about
  someone else's hardware written into a job name. The legs print the ladder they were handed
  instead.
- Windows joins Linux and macOS as an equally first-class target, on both x86_64 and
  arm64. Nothing in `src/arch/` changed to get there - it already dispatched on
  `target_arch` alone, never on the operating system, so the same NEON and SSSE3
  kernels this crate already shipped simply had never been proved running natively
  under Windows.

  They now are. `.github/workflows/native.yml` runs the full soundness differential,
  the kernel-agreement census, and the `survey` economic gate on real native silicon
  across all six Windows/Linux/macOS × x86_64/arm64 targets on every push - never
  emulated, never cross-compiled - and gates `cargo publish` on all six passing
  uncached. The matrix exists to keep proving that a row's numbers hold on the machine
  running them rather than to assume it, and it collected inside this same release: the
  six cells disagreed, so `price::MINTED` is now keyed by `(os, architecture, kernel)`
  and carries ten rows rather than two. A target still absent from the matrix declines
  with `BuildError::Uncalibrated` instead of inheriting another platform's numbers.
- `Screen` is a matcher that uses a sieve when one pays and the engine alone when one does
  not. `Screen::new` returns an error only for a pattern that does not parse; every economic
  and structural refusal is absorbed, and `is_match` answers exactly what `regex-automata`
  answers either way.

  `Sieve` hands back a `Result` whose *common* variant is refusal, and that is honest — most
  patterns should not be fronted by a filter. But it made the ordinary outcome of
  `cargo add sheng` an error a caller has to write code around, for a speedup they then do not
  get, and the rational response to that is to remove the dependency. Which is the wrong
  outcome twice over, because the decline is a fact about **this** pattern on **this** machine
  over documents of **this** length, and every one of those changes without the caller doing
  anything: a slate grows, a machine gets a row, a corpus moves from cache to memory. A caller
  who removed the crate never finds out.

  Nothing is hidden by absorbing it. `Screen::sieve` hands over the sieve and its arithmetic,
  `Screen::declined` hands over the refusal verbatim, `Screen::armed` says which of the two
  happened, and `Screen::dfa` exposes the automaton for a caller who needs a position rather
  than an existence answer. What is removed is the obligation to *handle* it.

  One automaton serves both roles, which is the arrangement `Sieve::of_dfa` recommends and
  this type makes automatic: the sieve is priced against the very engine that will confirm its
  survivors, so the gate's rival term measures the search that is really going to run rather
  than one like it. `tests/screen.rs` is mostly a single differential against the engine over
  synthetic bytes drawn from each pattern's own alphabet and over real source text, since a
  type whose whole purpose is to be indistinguishable from the engine is worth exactly what
  that indistinguishability is worth — and it separately asserts that an armed screen really
  refutes, because the differential would pass just as happily with the sieve wired to
  `false`.
- `Sieve`'s documentation has always told callers that one immutable instance serves
  every document and every worker, and until now nothing but auto-trait inference kept
  that true. `Send` and `Sync` are derived from a type's fields, which is exactly what
  makes them possible to lose in silence: one `Rc` handle, one `Cell` memoizing a probe,
  one raw pointer into a mapped table, and the promise is gone - with the breakage
  landing on the caller who believed the documentation rather than on the commit that
  took it away. Every public type is now named in a compile-time assertion, so that
  failure lands where it is caused.

  `Sieve` and `BuildError` are held to `'static` on top of it, because they are the two a
  caller moves *into* a worker rather than merely shares with one - a sieve built once and
  sent to a pool, an error carried back across a join - and neither could do that while
  borrowing from the automaton it was built from. The assertion is a `const` item rather
  than a test, for the same reason the `no_std` job builds bare-metal targets: the
  guarantee is not conditional on a test build. It is checked in all four feature
  combinations and on every target this crate compiles for, including the ones with no
  harness to run a test with and no threads to spawn.
- `price::MINTED` goes from two rows to ten, and every kernel this crate implements for real
  silicon is now priced on at least one machine it runs on. `.github/workflows/mint.yml` was
  dispatched across all six legs `native.yml` proves correctness on — Linux, macOS and Windows
  on x86_64 and arm64 — against one 60.6 MiB corpus in one session, which is the only
  arrangement in which these numbers may be divided by each other.

  The consequences are dispatch, not decoration. x86_64 machines stop running the 16-byte SSSE3
  path because AVX2 was implemented but unpriced: on the Linux runner AVX2 costs 0.325 ns/B
  against SSSE3's 0.366, and on the Intel Mac 0.527 against 0.922. `price::DORMANT` is down
  from three entries to one — AVX2 and AVX-512 both have rows, and `Simd128` remains the only
  kernel no mint leg can reach, because a `wasm32` row's nanoseconds belong to the runtime and
  the host under it.

  ## AVX-512 is priced, and loses

  The first AVX-512 row this crate has ever had reads 0.458 ns/B where the same machine's AVX2
  reads 0.376 and its SSSE3 reads 0.437 — the 64-byte shuffle is the slowest of the three rungs
  on the only machine that offers all three. That is the row cost-ordered dispatch exists for:
  AVX-512 is *correct* here, proven under Intel SDE on every CI run, and correct-but-slower is
  exactly what a register-width prior cannot see. It is priced and simply not elected.

  ## Seven of the ten rows carry no cache-resident column

  All six legs timed both regimes; four came back with a cache-resident column *worse* than the
  memory-resident one, which describes a shared runner's scheduler rather than a memory system.
  `examples/mint.rs` detects the inversion and emits the whole column as `0.0`, so those
  machines decline a caller who declares `Residency::Cache` instead of arming on noise.

  ## One key, two machines that disagree

  `macos`/`aarch64` is the row that did not come from this session, and the reason is worth
  stating rather than hiding. Its mint leg priced a 3-core runner at `dfa_excursion` 14.00
  against `skip_excursion` 10.18; pasted in, `examples/survey.rs` on a 16-core Apple laptop
  armed `(?-u)panic!\(` and measured the result at 0.566x — a sieve running 1.8x slower than
  the engine it displaced. Every machine in the session read those two coefficients 20–30%
  apart; that laptop reads them within 1%, which is a claim about a cache hierarchy and
  precisely what `(os, arch)` cannot key on.

  So the shipped row is the laptop's, on the rule that when one key covers machines that
  disagree, the row that ships is the one overstating the engine's disadvantage least — the same
  asymmetry that makes `mint` record the *higher* of two paired excursions. An under-armed sieve
  costs a win nobody sees; an over-armed one ships a measured slowdown, and only the second is a
  failure `survey` will name.
- `price::Residency` — a required third fact on every `Policy`, naming where the bytes
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

### Changed

- Dispatch elects the kernel this machine's own rows price *cheapest*, where it used to
  elect the widest kernel that had a row at all. Register width was standing in for speed,
  and x86_64 has now refuted the substitution: a mint on a four-core Windows runner timed the
  AVX-512 composition pass at 0.458 ns/B against AVX2's 0.376 and SSSE3's 0.437, so the
  64-byte shuffle lost to both narrower rungs on the machine that has it. Under the old rule,
  landing that row would have moved every such machine onto the *slowest* of the three, and
  the row proving it slowest is the row that would have done it.

  `arch::available` still returns the ladder widest-first and still ends in `Scalar`, but
  that order is now explicitly a prior: it decides nothing where a measurement exists,
  breaks ties where two rows agree, and answers on a machine nothing has priced. What ranks
  the rungs is `sieve_per_byte` at `MAX_CONJUNCTS`, read from `price::MINTED` for the
  running `(os, arch, kernel)`. `minted.rs`'s dispatch test asserts the same rule rather
  than the width order it used to assert, so a wider-but-slower kernel cannot win a dispatch
  without failing the build.

  This also removes the last reason to keep a measured kernel dormant, and `price::DORMANT` is
  down to one entry because of it. A kernel that loses on the machine that measured it can now
  be priced and simply not chosen there, which is a fact in `MINTED` rather than an absence
  from it — and on a machine where it wins, it wins.
- Every claim this project makes now has one home, and the copies that used to restate it
  point at that home instead. The soundness argument, the arming inequality, the start-block
  skip's exactness, the operating-system column in `price::MINTED`, and the prior-art
  bibliography were each told in full in three or four places — README, crate docs, a module
  `//!`, a nested `README.md` — which is how they came to disagree. The crate-root
  documentation is roughly half its previous length and keeps the pitch, the compiled usage
  example, and pointers into the modules that own the contracts; the nested `src/**/README.md`
  files are filesystem maps again rather than second Design essays.

  The disagreements the duplication had already produced are gone with it. `price::MINTED`
  was described as keyed on `(architecture, kernel)` in the crate docs and on the triple
  everywhere else; `SECURITY.md` asked reporters for a `Policy::default()` that no longer
  exists, `Policy::new(residency)` having replaced it; `examples/mint.rs` still called a price
  row a claim about a pair. Absolute nanoseconds-per-byte figures appeared in four documents
  at two different pairs of values, so they now appear only beside the `Calibration` constants
  they annotate, where a re-mint edits them in the same motion it edits the number.

  What is left in prose is checkable or named. The README cites `MAX_CONJUNCTS` rather than
  the digit two, and a compile-time assertion in `lattice.rs` holds that constant and `LANES`
  to the sixteen-lane, two-quotient shape the prose describes, so moving either becomes a
  build failure rather than a silent falsehood. This is the guard the `#[cfg(doctest)]` README
  include already provides for the Rust blocks, applied to the two numbers that were most
  exposed without one.
- The published `.crate` now carries the crate and nothing that merely operates the
  repository that produces it. `.github/` alone was roughly two fifths of the tarball -
  nine workflows, a triage script, and a label registry - joined by the git hooks, the
  release-please and towncrier configuration, and the unreleased `changelog.d/`
  fragments, none of which a consumer holding only the tarball can run or act on. What
  ships is now a declared whitelist rather than a subtraction from whatever the working
  tree happens to hold: sources, the tests and examples that argue they are right, the
  license trail, and `clippy.toml`/`rustfmt.toml`/`deny.toml` so a downstream rebuild's
  own checks agree with this repository's. That is half the file count and roughly
  thirty percent less on the wire.

  `rust-toolchain.toml` is deliberately among the departures: it pins 1.96.0 so this
  repository's contributors format and lint identically, which is no business of someone
  rebuilding the extracted source, and it would have quietly overridden the 1.95
  `rust-version` the sources actually ask for.

  Two smaller omissions closed alongside it. `no-std` joins the crate's crates.io
  categories - `--no-default-features` is the build the feature table was designed
  around, so the audience browsing that category is the one this crate was shaped for -
  and a `[package.metadata.docs.rs]` block now builds the documentation with every
  feature on, for both x86_64 and aarch64 because the crate's risk surface is two
  hand-written kernels and NEON is half of it. It passes `--cfg docsrs`, which turns on
  a `doc_cfg` leg in `src/lib.rs` so the pattern constructors and the `Dfa` trait are
  labeled with the `regex-automata` feature that carries them instead of rendering as
  though they were unconditional.
- The rule-slate case — one refutation retiring many searches — was the workload this crate
  named as its best and never measured. `tests/slate.rs` now measures it, and the walls it
  found are documented where a caller plans against them rather than discovered after.

  **If the rules carry literals, the union already is the answer.** Sixty-four
  literal-prefixed rules measure 11.96 ns/B as separate engines and 0.12 as one union — the
  fan-out almost exactly, because the union keeps a multi-literal accelerator and still pays
  one pass's price. A sieve in front of that retires nothing, and `Bypass::Slate` is how the
  gate is told so. What bounds the union is construction rather than throughput, which
  `Bypass::Slate` now states in figures: 12.6 KiB, 4.5 MiB and 65 MiB of dense table at 1, 64
  and 256 rules, builds of 0.2 ms, 0.75 s and 114 s, and no determinization at all past 256
  inside a gibibyte.

  **A slate's own union stops being sieveable almost immediately.** Over eight literal-free
  rules of the kind a secret scanner is made of, the union's reachable core passes
  `MAX_CORE_STATES` by the seventh and the lattice stops finding a register-sized closed
  partition at the *second*. A 16-block quotient of 1,200 rules is not a filter that lost on
  price; it is a filter that does not exist. What does exist is a coarse skeleton of one
  *family* — `[0-9]+[-./:][0-9]+[-./:][0-9]+` contains every SSN, card number, date,
  timestamp and version string in nine states — which is why `Sieve::of_superset_with` takes
  an automaton and not a pattern list. The test is written as a search asserted in the
  direction that catches the claim getting *better*, so a lattice that ever harvests wider
  fails it and the prose it justifies has to be rewritten.

  **Every slate size converges on the same ceiling**, so record length rather than rule count
  is what decides. That skeleton is worth at most 11.3x over 256-byte records, 3.2x over a
  kilobyte and 1.00x over 16 KiB — the same filter, the same slate, three different answers.
  The slate regime is packets and log lines; over large documents there is nothing to win.

  `examples/bench.rs` now times the engine beside the kernel across record lengths instead of
  the kernel alone, which is what `VALIDITY_FLOOR` is actually a claim about — the gate
  compares a ratio, and the sieve's own curve cannot settle where a verdict stops travelling.
  Swept, the sieve's edge over a walking rival holds within a couple of percent from 64 KiB
  down to a kilobyte, is 16% under nominal at 256 bytes, and 39% under at 128: the floor sits
  exactly where the model's error crosses `MARGIN`, measured rather than argued.

  That sweep also retired the fix it was expected to justify. Minting per-call constants
  would not have lowered the floor, because the larger short-record effect runs the other
  way: consecutive searches over short records are independent dependency chains that a wide
  core overlaps, so the *rival* gets cheaper per byte — 1.27 ns/B over 4 KiB records against
  0.71 over 64-byte ones. That is a reorder window saturating rather than a coefficient, and
  a number fitted to one machine's window describes no other machine's. A floor states the
  same fact without claiming a portability it does not have.
- `price::MINTED` is keyed on `(os, architecture, kernel)`. It was keyed on the pair, on
  the argument that an OS column would be warranted "the day some target's own measurement
  disagrees, not before". That day arrived the first time `.github/workflows/native.yml`
  ran on every push rather than only before a publish.

  Three of its six legs were pricing themselves from a row minted on a fourth machine, and
  `examples/survey.rs` caught every one of the three arming a pattern that then lost
  against real source text - macOS x86_64 by 3%, both `aarch64` servers by 8%. The mint says
  why: that macOS box times its own AVX2 sieve at 0.527 ns/B where the Linux row it was
  borrowing reads 0.325, while the DFA walk the sieve is weighed against differs by under
  2%, so the ratio that decides arming moves with the machine. The instruction
  set fixes what the kernel *is*; the cache hierarchy fixes what it costs against its
  rival. The three legs that passed had been lucky in their silicon rather than vindicated
  by it.

  `Calibration` therefore gains an `os` field and `BuildError::Uncalibrated` an `os`
  member - the decline has to name all three parts, because `MINTED` holding a row for your
  architecture *and* your kernel and still not pricing your machine is now the ordinary
  case rather than a bug in resolution. `price::OS` is the string to name, and it
  enumerates only the five operating systems this repository builds: an OS nobody has
  minted on reads `"unknown"`, matches nothing, and declines, which is what an unenumerated
  one already did.

  `examples/mint.rs` prints the column, and now withholds one it could not measure. A busy
  runner that times a cache-resident haystack as costing the engine *more* than a
  memory-resident one has measured a loaded machine rather than a memory system, so the
  whole cache column is emitted as `0.0` - callers declaring `Residency::Cache` on those
  rows get `Uncalibrated`. The whole column, not the inverted coefficient: `is_measured`
  reads `dfa_skip` alone, so zeroing a lone inverted `dfa_excursion` would leave the regime
  looking measured while pricing the engine's excursion at free, which is an over-arming
  row.

### Fixed

- Both workflow fixes in the previous entry landed correctly and neither one reached the run it was
  written for, for a different reason each.

  `miri.yml` stopped aborting and started taking hours. The `cfg(miri)` answer for `CPUID` let the
  leg get past `arch::available` and on to `selectivity::tests`, whose three cases all build a
  `regex-automata` DFA through the module's own `quotients` helper — the exact cost the job's own
  header says it excludes. The skip list named one of the three by prefix, so two ran under the
  interpreter, and the leg went from 87 seconds to still-running with nothing failing. The skip is
  now the module, and the job carries `timeout-minutes: 20`, so the next test that grows a
  determinization is a red job in twenty minutes rather than a queue slot held all afternoon.

  `release-please.yml`'s fold job stopped dying on the push and started not running. It was gated on
  release-please's `pr` output, which is reported only by the run that creates or updates the PR: the
  push carrying the push fix changed nothing release-please acts on, so the output was empty, the job
  was skipped, and the open PR kept sitting red on the stale `Cargo.lock` this job exists to move —
  the failure mode the gate's own comment says it prevents, since a fold has to be able to re-run
  while the PR stays open. When the output is empty the job now asks for the open PR release-please
  labels `autorelease: pending` and folds onto that.

  Running on every push is what exposed the fold as incremental. towncrier refuses to write a second
  section for a version it has already written, so the second run over an already-folded branch was
  a hard error rather than a no-op — and fragments that reached main after the first fold could not
  have been folded at that version even if it had been. The job now restores `CHANGELOG.md` and
  `changelog.d/` from main before folding, which makes the result a function of main at that version:
  idempotent when nothing moved, complete when something did. Its "nothing to fold" check compares
  against `HEAD` rather than the index, since restoring those two paths stages them and the previous
  comparison would have called a real change quiet.
- The AVX2 and AVX-512 composition kernels kept their four chains on the stack instead of in
  registers. Both wrote `for (reg, (f, h)) in compose.iter_mut().zip(&mut high).enumerate()`,
  and in a release build that zip did not inline — `Zip::new` survives as a call — so LLVM
  declined to unroll the four-way loop, which made `[__m512i; 4]` and `[__m256i; 4]`
  addressable and spent a full-width spill and reload on every chain every step:

  ```asm
  vpshufb   (%rax,%rbx), %zmm0, %zmm0   ; compose[reg], loaded from the stack
  vmovdqa64 %zmm0, (%rax,%rbx)          ; and stored straight back
  vpmaxub   (%rax,%rbx), %zmm0, %zmm0   ; high[reg], likewise
  vmovdqa64 %zmm0, (%rax,%rbx)
  ```

  The op count is not the injury. `compose[reg]` is a *recurrence* — each step composes onto
  the previous one — so a spill puts store-to-load forwarding on the one dependency the
  kernel is built to keep inside a register, on a target with thirty-two `zmm` registers and
  eight of them wanted. AVX-512 additionally called out to `core::array::from_fn`'s `FnMut`
  machinery eight times per hot loop to build a `[*const u8; 4]` of row pointers.

  Both now index a constant trip count, and `quad` takes four pointers the way AVX2's `pair`
  always took two. Measured in the emitted assembly of a real release build, per kernel:
  spill stores 28 to 7, and the wide `vpshufb` count — the tell for whether the four-chain
  loop unrolled at all — from 2 to 8.

  This was found while asking why AVX-512 measured *slower* than AVX2, 0.335 against
  0.290 ns/B, despite consuming four times the bytes per step — and it is worth recording that
  it turned out not to be the answer. Both kernels got faster once their chains stayed in
  registers, and the ordering between them did not change: re-minted on real silicon they read
  0.458 against 0.376 ns/B, the same ranking with a wider gap. So this is a fixed bug and a
  ruled-out hypothesis, not an explanation; what remains unexplained is left unexplained in
  `price::WINDOWS_X86_64_AVX512` rather than guessed at. `ssse3`, `neon` and `simd128` were
  never affected and are unchanged — one row per register is an indexed read LLVM already
  folds, which is exactly why the fault looked like a property of AVX-512 instead of a
  property of how the loop was written.

  Nothing about what these kernels compute has changed: same indices, same order, same
  operations, so `tests/kernels.rs` holds them to the scalar reference as before — on native
  silicon for AVX2 and under Intel SDE for AVX-512.
- The `survey` gate used to read the clock's own resolution as a verdict about the
  cost model. It timed each arm once as a min-of-five and asserted that every armed
  row cleared 1.000x, which is the right claim over a real corpus and nonsense over
  a small one: run against this crate's own 0.3 MiB of source, three rows came out
  at 0.62-0.95x and the assertion announced that the model admitting them was
  wrong. The same three rows measure 1.07-1.50x over 22.6 MiB. Nothing was wrong
  with the model; the instrument was reporting noise and a cache-resident corpus as
  evidence.

  Two changes, both narrowing what the gate is willing to claim rather than what it
  enforces. Every arm is now five samples of a min-of-five, so a row carries an
  interval instead of a number, and only a row whose whole interval sits below
  1.000x counts as a loss - one that straddles is printed as undecided and asserts
  nothing. And below 8 MiB of corpus the survey declines to judge the model at all,
  because a calibration is nanoseconds per byte read from memory and a corpus that
  fits in cache never reads from memory, so the engine's own `memchr` accelerator
  runs at tens of gigabytes a second and beats every price the crate knows for
  reasons that have nothing to do with the sieve.

  A row that genuinely loses still fails the run, and now says so legibly - the
  ratio, the interval, and both arm times, instead of a debug dump of raw seconds.
- The arming gate now requires a modeled speedup to clear 1.0 by `price::MARGIN` (25%)
  rather than by any amount at all. Two patterns on the survey slate were arming on an
  edge smaller than the measurement error of the coefficients that produced it, and then
  losing.

  `WalletService` scored 1.010x and `foo[^\n]*bar` scored 1.009x. Both elect a `memchr`
  skip over **the same byte the engine already accelerates on** — escape set `['W']`
  against start-state accelerator `"W"`, `['f']` against `"f"` — so the streaming halves
  of the two prices are the same loop over the same needle and cancel exactly. What was
  left carrying the verdict was `skip_excursion` sitting 3.5% under `dfa_excursion`, and
  `price::MACOS_AARCH64_NEON` publishes those coefficients' own run-to-run spread as
  ~10% and ~21%. The gate was betting on a difference the mint cannot resolve.

  It lost the bet about half the time, which is what a 1% prediction from ±20% inputs
  should look like: measured end to end, the two came out **0.780x and 0.940x** against a
  cache-resident corpus and **1.119x and 1.057x** against a memory-resident one. Neither
  figure was ever outside its own measurement interval — `WalletService` read
  0.978-1.250x on 22 MiB, which is a survey declining to say who won.

  Scale invariance is what makes this a real gap rather than a conservative nudge. The
  gate is provably indifferent to a factor common to every coefficient — clock, thermal
  state, ambient load — so the residual it *is* exposed to is exactly the part that is
  not common: the spread between two independently timed kernels. A margin is the only
  place that can be accounted for, and the asymmetry settles its direction. A sieve that
  declines costs the caller the speedup it would have had and nothing more; a sieve that
  arms on noise costs a full sieve pass on every document it then fails to refute, for as
  long as it is deployed. So the margin is required of the sieve rather than split.

  What it costs and what it buys, on the same slate and machine:

  - 0.3 MiB, cache-resident: geomean over armed rows **1.565x → 2.183x**, and two rows
    that measured below 1.0 are gone.
  - 22.3 MiB, memory-resident: geomean **2.277x → 3.346x**, and the survey now reaches
    "every armed row cleared 1.000x" instead of reporting one row as undecided.
  - Given up: 1.119x and 1.057x on 22 MiB, both inside their own intervals.

  `skip.rs` has always stated the rule in prose — "a rival already `memchr`-ing the
  identical byte cannot be beaten by a filter that has to find the same byte first" — and
  `tests/policy.rs` now enforces it, checking both that the two sets coincide and that
  the pattern declines. Deliberately as a test and not as a code path: a hard "same set,
  therefore decline" would be **wrong**, because the two loops search the same needle but
  excurse into different machines — the engine into a dense DFA whose table misses cache,
  the sieve into sixteen blocks already in registers — so a genuinely cheaper excursion
  is a genuinely cheaper sieve, and vetoing it would veto a real win. Coinciding escape
  sets do not make a sieve worthless; they cancel the streaming term and leave the
  verdict resting on one ratio of two noisy coefficients. That is a reason for the margin
  to exist, not a second gate.

  `BuildError::NotWorthIt` now names the bar it missed, so a decline reads
  `1.010x, under the 1.25x a measured decision needs` rather than leaving a reader to
  wonder why a number above 1.0 was refused.
- Two workflows outside `ci.yml` were failing while `ci.yml` was green, which is the arrangement
  that lets a red badge stop meaning anything.

  `miri.yml` has been red since the AVX-512 probe landed, and not because of anything it found.
  Miri does not implement `CPUID` and aborts on it, so `arch::available` took the whole run down
  from inside a calibration test. `x86::leaves` now answers `0` under `cfg(miri)` — the honest
  answer for an interpreter where no leaf is readable — and because every caller gates its own
  `__cpuid` behind a `leaves()` comparison, that one `cfg` disarms the entire probe and leaves
  the leg on the scalar path its name promises.

  `release-please.yml`'s fold job checked out with `persist-credentials: false` and then pushed
  with `git push origin`, which authenticates with the credential that flag exists to withhold.
  It passed a `token:` to the checkout as though that supplied one, so the job would fold the
  changelog, commit it, and die on the push — visible only once there were fragments to fold,
  since an empty fold exits before pushing. The push now carries the App token in a one-off
  remote URL, and the branch name and token both arrive through the environment rather than
  interpolated into the shell.
- `CostFact::pays` compared two costs without first checking they were costs, and a negative
  one armed a sieve. With a filter leaky enough that survival is ~1 the left side is
  `sieve + rival`, so `(sieve + r)(1 + MARGIN) < r` holds for any `sieve` under `-MARGIN * r`
  — the inequality inverts and admits a filter that retires nothing, which is the single
  outcome the whole `price` module exists to prevent.

  Nothing internal could produce such a number, which is why it survived: every coefficient
  reaching the gate came from a mint, and `Calibration::is_measured` already refuses a row
  whose walk or skip price is not positive. It became reachable the moment a caller could
  state a rival's price outright, and it was reachable before that through a hand-built
  `Calibration` — the seam `tests/policy.rs` exists to keep open.

  So the guard is at the comparison rather than at either entry point, where it covers every
  source of the defect instead of the two that are spelled out: `pays` now requires the total
  to be a non-negative real and the unfiltered side to be finite before comparing them. NaN
  comes along for free, every ordering on it already being false, as does the `0 * infinity`
  the survival term produces for a perfectly selective filter against an infinite rival.
  Verdicts on real coefficients are unchanged — the guard only ever rejects inputs that were
  never prices.
- `Decline::MatchesEmpty` was checking `dfa.is_match_state(start)` directly, but
  `regex-automata`'s start state is never itself a match state - not even for a
  pattern that matches the empty string - because it encodes "about to try",
  not "already matched"; the empty-match acceptance only shows up one step
  later, at the end-of-input transition (exactly the `Row.accepts` computation
  a few lines below it already uses). The guard could not fire for any pattern,
  on any input, ever - a property test sweeping thousands of generated patterns
  through `Sieve::new` turned this up directly, by fuzzing the one input the
  existing suites always assumed was already well-formed: the pattern itself.

  In practice every pattern that matches empty still declined - via the lattice
  harvest finding no discriminating quotient and returning `NoQuotient` instead -
  so this was never a soundness gap, only the wrong reason and a wasted harvest.
  `Projection::of` now checks `next_eoi_state(start)` too, and
  `a*`, `.*`, the empty pattern, and `a**` all now decline with the specific,
  documented `MatchesEmpty` reason instead of the generic `NoQuotient`.

  Added `tests/errors.rs`: a property suite over the build path specifically,
  sweeping thousands of generated and garbage pattern strings through
  `Sieve::new` to prove it never panics, plus directed coverage proving every
  `BuildError`/`Decline` variant is actually reachable, explains itself with the
  crate's shared `"no sieve: …"` prefix, and reports no `source()` it doesn't
  have.
- `Decline` is exported from the crate root and is the error type of the public
  `Projection::of`, but it only derived `Debug` — a caller composing it with `?`
  into `Box<dyn std::error::Error>` (the exact shape this crate's own top-level
  doc example uses) would not compile, and `BuildError::Shape` had to fall back
  to `{d:?}` to print one at all. `Decline` now carries the same `Display` +
  `std::error::Error` pair `BuildError` already does, with one prose line per
  variant, so both of the crate's public error types answer `?` and `.to_string()`
  identically instead of one of them being a `Debug`-only enum wearing an error's
  name. `BuildError::Shape`'s message now reads through `Decline`'s own `Display`
  rather than its `Debug` form.
- `price::DORMANT` is keyed on `(os, arch, kernel)` instead of on the kernel alone, because
  one column short it could not describe the state x86_64 is in. Its entries are now a
  `price::Dormant` struct whose `os` and `arch` are `Option`s, where `None` means every
  machine.

  GitHub's Linux x86_64 fleet is not uniform. `mint.yml`'s `linux-x86_64` leg drew a runner
  whose `available()` came back `[Avx2, Ssse3, Scalar]`, and an hour later a `ci.yml` runner
  on the same label reported AVX-512 present — so `linux`/`x86_64` is simultaneously a
  machine that can run the kernel and a machine no mint has priced it on, while
  `windows`/`x86_64` prices it outright. A kernel-only list has to call AVX-512 either priced,
  leaving the Linux box unaccounted for, or unpriced, contradicting the Windows row. Both are
  false, and the per-machine audit failed on exactly that.

  The audit reads the machine on both sides now: a vector kernel this silicon can run must be
  priced or covered by an entry that speaks for this machine, and an entry that speaks for
  this machine must not turn out to be priced on it. Landing a row still deletes a line.

  `tests/policy.rs`'s coinciding-accelerator test also stops asserting a decline flatly.
  Where the sieve's leaves are the engine's own accelerator set the streaming halves of the
  two prices cancel, leaving the verdict on the ratio of two excursion coefficients — 3.5%
  apart on the one machine the assertion was written against, and 20–30% apart on five of the
  six minted since. The margin cannot dismiss that and should not, so the test now requires
  the decline *unless* this machine's own pair clears `MARGIN`, and still fails an arming that
  rests on coefficients only noise separates. `windows`/`aarch64` arms, and
  `examples/survey.rs` is what adjudicates it against real source.


## [0.1.0] - 2026-08-03

### Added

- A skip kernel, so the sieve stops reading bytes it provably learns nothing
  from. While a run sits in the quotient's non-accepting start block, every
  self-loop byte is a no-op, and unanchored patterns spend nearly all their
  time there — 98% of corpus bytes for `WalletService`, 99% for
  `#[0-9a-fA-F]{6}`. `skip::Skip` finds the next byte that actually leaves the
  block with `memchr` for escape sets of width 1-3 and a nibble classifier
  (Muła's shufti, as ripgrep and Hyperscan use it) for 4-128 ASCII, then the
  kernel jumps straight there. The classifier is exact rather than a prefilter:
  `lo[b & 0xF]` carries one bit per high nibble and `hi[b >> 4]` selects it,
  which is only the truth while the set is ASCII, so a set with a high byte or
  over 128 values is refused outright rather than approximated.

  Per-lane, not per-crate. Skipping is 8.8-11x the composition kernel on a
  one-byte escape set and 0.25x on a three-way alternation, because a scalar
  excursion loses badly to four lanes advancing at once — so `Lane::plan`
  prices both against the calibration and takes the cheaper, and the sieve runs
  a mix. On the survey slate the planner's choice matches the measured winner
  on 8 of 8 patterns, declining the skip in exactly the four cases where it
  would have cost 1.4-4x. Pricing needed one new calibration coefficient,
  `skip_excursion`, minted per instrument because the sieve's excursion out of
  a skip is far cheaper than the engine's out of an accelerated DFA; `mint` now
  measures every ratio interleaved against its own baselines, since an
  excursion timed against a baseline from ten minutes earlier measures the
  afternoon's contention rather than the excursion.

  End to end (`survey`, 3000 documents / 22.6 MiB, M4; `SHENG_NO_SKIP=1`
  reproduces the baseline): the four patterns that already armed go 2.831x to
  3.111x geomean, almost all of it `[0-9]{3}-[0-9]{4}` at 3.211x to 4.423x. Two
  that could not previously pay for themselves now arm — `WalletService` at
  1.158x and `foo[^\n]*bar` at 1.086x — which is why the headline geomean over
  _armed_ rows reads lower at 2.214x: the set it averages grew by two marginal
  wins. `<[^>]*>` and `#[0-9a-fA-F]{6}` got 7x cheaper to sieve (0.188 to 0.026
  ns/B) and still correctly decline, because the engine's own accelerator is
  just as fast on them.

  The unsafe SIMD is differentiated against a scalar definition of the same
  set: every byte value against every set shape, 2048 pseudo-random ASCII sets,
  and a planted escape at every offset of every length from 1 to 72 — that last
  one is what catches a vector search dropping its remainder, which fails
  silently as a missed match rather than a crash. The soundness suite now
  asserts a skip lane was actually chosen before comparing, and draws half its
  haystacks from a narrow alphabet, because uniformly random bytes leave the
  start block immediately and never exercise a long jump or its tail.
- Per-machine calibration instead of one laptop's numbers. price::MINTED
  carries a row per (architecture, kernel) pair — arm64 macOS and native x86_64
  Linux, both minted over one frozen 60 MiB corpus — and price::active()
  resolves the running target against it. A machine with no row declines every
  pattern with BuildError::Uncalibrated rather than inheriting foreign ratios,
  which matters because the two ISAs disagree about twofold in opposite
  directions: skip/walk 0.014 vs 0.026, sieve/walk 0.395 vs 0.196. Absolute
  speed provably cannot move a decision
  (scaling_the_whole_calibration_changes_no_decision).
- Policy — one replaceable home for every empirical fact the arming gate rests
  on: the calibration, the byte-class chains, the per-byte marginals and the
  nominal document length. Sieve::with(pattern, &policy) is the seam;
  Sieve::new is Policy::default(). Callers whose corpus is not source code, or
  whose silicon nobody measured, mint their own with the mint example instead
  of living with someone else's defaults.
- shuffle::kernel() reports which of NEON, SSSE3 or scalar dispatch actually
  chose, and dispatch itself now reads it, so the two cannot disagree. The
  vector/scalar differential test asserts a vector kernel really ran —
  previously, a target that fell back to scalar would have compared the
  reference against itself and passed vacuously. Both vector paths are now
  exercised: NEON natively, SSSE3 under Rosetta and natively on x86_64 Linux.

### Changed

- Point the origin note at the standalone irregex repo instead of an in-tree
  path.
- Selectivity estimation aggregates the 256 bytes into class edges once, in a
  new `Spread`, instead of re-walking all 256 of them on every one of the 512
  power iterations. The joint (block, class) chain factors into the
  class-to-class draw and a per-(block, class) spread over destination blocks,
  so the inner loop visits only the transitions that exist. Predictions are
  unchanged; the phase drops from 44-75 ms to 0.16-0.42 ms, which takes a whole
  sieve build from 44-75 ms to 0.19-0.55 ms — the build path was 99.9% this one
  function. Convergence is still run to the full iteration count rather than
  exiting early on a settled bulk: the accepting mass is what the estimate is
  about, it can be 1e-29, and it is still accumulating long after the
  distribution looks stationary.
- The README is restyled into the house voice, which changes how every reader
  meets the
  crate. The five tables become bulleted lists with bolded lead terms, every
  paragraph is
  held to three sentences, headings are labels rather than arguments, and bold
  is confined
  to the one job it keeps. Three sections that did not exist are now there: a
  contents
  list, where a bug or a vulnerability should be reported, and who should be
  using
  `regex-automata` or `gist` instead of this crate. No claim, number, citation,
  or path
  changed - the restyle was checked token by token against the text it
  replaced.
- The register kernel holds the transition function instead of the state, so
  the shuffle chain no longer waits on itself. Seeded with the identity, one
  shuffle per byte composes rather than steps, which makes the loop independent
  of where the scan is and lets the haystack split into four slices the shuffle
  unit can advance at once — the enumerative technique of Mytkowicz, Musuvathi
  & Schulte (ASPLOS 2014), which costs nothing here because a sieve has already
  capped its machine at sixteen blocks. Same instructions per byte, 0.346 to
  0.132 ns/B geomean on an M4 (2.6x), and faster at every document size from 64
  B up. The accept test stays exact: each slice's per-lane max is read at the
  lane the real trajectory entered it on, so the parallel kernel refutes the
  same documents the scalar reference does rather than a subset. The macOS
  aarch64 calibration was re-minted against it, which arms `panic!\(` where the
  old sieve price declined it and takes the survey slate from three armed rows
  at 1.53x geomean to four at 2.85x.

### Fixed

- The `survey` gate used to read the clock's own resolution as a verdict about
  the
  cost model. It timed each arm once as a min-of-five and asserted that every
  armed
  row cleared 1.000x, which is the right claim over a real corpus and nonsense
  over
  a small one: run against this crate's own 0.3 MiB of source, three rows came
  out
  at 0.62-0.95x and the assertion announced that the model admitting them was
  wrong. The same three rows measure 1.07-1.50x over 22.6 MiB. Nothing was
  wrong
  with the model; the instrument was reporting noise and a cache-resident
  corpus as
  evidence.

  Two changes, both narrowing what the gate is willing to claim rather than
  what it
  enforces. Every arm is now five samples of a min-of-five, so a row carries an
  interval instead of a number, and only a row whose whole interval sits below
  1.000x counts as a loss - one that straddles is printed as undecided and
  asserts
  nothing. And below 8 MiB of corpus the survey declines to judge the model at
  all,
  because a calibration is nanoseconds per byte read from memory and a corpus
  that
  fits in cache never reads from memory, so the engine's own `memchr`
  accelerator
  runs at tens of gigabytes a second and beats every price the crate knows for
  reasons that have nothing to do with the sieve.

  A row that genuinely loses still fails the run, and now says so legibly - the
  ratio, the interval, and both arm times, instead of a debug dump of raw
  seconds.
- The examples and the soundness suite no longer assume the crate's address.
  Corpus discovery climbs to the enclosing checkout and honors SHENG_CORPUS,
  replacing a walk rooted at the process CWD and a relative path hardcoded to
  the crate's old home — either of which silently read the wrong tree once the
  crate moved or was published standalone. Provenance comes from std rather
  than uname and date subprocesses, so a mint works on any target that can run
  the crate.
- Two policy tests were asserting nothing, and one of them was asserting the
  opposite of the model. `shorter_documents_are_harder_to_justify` skipped any
  pattern that declined at the nominal length — which was every pattern on its
  slate — so its loop body had never once executed. Teaching the sieve to skip
  armed two of them, the assertion ran for the first time, and it disagreed
  with `price::survival` immediately: survival is `1 - (1-f)^len`, the
  probability that _at least one_ position falls through, so it rises with
  length and a **longer** document is the harder sell. Renamed, inverted, and
  given a census that fails if it ever goes vacuous again.
  `a_caller_can_price_a_machine_the_crate_never_measured` lost its
  demonstration for a subtler reason — `[Ww]allet` now arms on both the shipped
  and the hypothetical calibration, since the sieve no longer needs the engine
  handicapped to be worth it, so nothing in its set moved. A hex-literal scan
  carries it instead. Both are the same lesson: a test that can quietly stop
  testing is worse than no test, so each now counts what it compared and says
  so.
