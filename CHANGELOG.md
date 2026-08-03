# Changelog

<!-- towncrier release notes start -->

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
