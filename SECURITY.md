# Security Policy

`sheng` decides that a document cannot match, and something downstream then
never looks at it. That makes the whole threat model one sentence: **a false
refutation is a missed match.** It does not surface as a crash or a slow query -
it surfaces as a scanner that reported clean, and it is indistinguishable from
there being nothing to find.

Treat that as the security boundary rather than as a correctness bug, because
callers put this in front of things that matter: a secret scanner, a rule slate
over a document stream, an intrusion filter. Refutation is the only claim this
crate makes, and it is the only one that can hurt you.

## Reporting a vulnerability

**Do not open a public issue, pull request, or discussion.**

Use GitHub's private reporting - the **Security** tab on this repository,
"Report a vulnerability" - which opens a thread only the maintainers can read.
If that is unavailable to you, email **<security@billylives.com>**.

Please include:

- the pattern, and the haystack bytes that were refuted when they should not
  have been (a script that builds them beats a tarball);
- the output of `sheng::shuffle::kernel()`, which names the vector path that
  actually ran, and your OS and architecture;
- the `Policy` in force if it was not `Policy::default()`;
- the crate version and how you built it.

We will acknowledge within **72 hours** and give you a triage verdict with a
severity within **7 days**. If it is real we will agree a disclosure date with
you, credit you in the changelog fragment and the release notes unless you would
rather we did not, and ship the fix before the details go public. There is no
paid bounty.

We will not pursue anyone who reports in good faith, works against their own
machines and their own data, and gives us a reasonable window to fix the thing
before publishing.

## Supported versions

Pre-1.0, and the version number says so. Fixes land on `main` and ship in the
next release; there are no maintained release branches and no backports to
earlier tags. Watch releases on this repository if you pin.

Pattern semantics belong to a neighbor with its own policy. This crate reads its
automaton from [regex-automata](https://github.com/rust-lang/regex) and never
parses regex itself, so anything about what a pattern means - parsing, Unicode,
match semantics - is theirs. Any tracker reaches us, and we will move a report
rather than bounce you.

## What we consider a vulnerability here

- **A document refuted that holds a match.** The one that matters. Every path
  below is only interesting because it can produce this.
- **A partition that is not closed.** Soundness rests on the substitution
  property holding for every byte. Input that gets a partition past the closure
  re-check and into a shipped sieve breaks the proof, not an optimization.
- **A classifier that answers "not a member" for a byte that escapes.** The
  nibble classifier covers `0x00..=0x7F` and is required to refuse every set it
  cannot represent exactly. A set that gets accepted and then approximated makes
  the skip loop jump over a real transition, which is a missed match wearing a
  performance win.
- **A skip that overshoots.** `skip::Skip` reports the next escaping byte, and a
  vector search that drops its remainder returns `None` where an escape existed.
  The tail of the last chunk is the sharp edge; it is fuzzed at every offset for
  exactly this reason.
- **A collapse that reads the wrong lane.** The parallel kernel resolves each
  slice's running max at the lane the real trajectory entered on. Reading a
  different lane under-reports an accept, and under-reporting an accept is the
  unsound direction.
- **Memory unsafety in the vector paths.** The files under `src/arch/` are
  hand-written intrinsics behind `unsafe`, dispatched into by `shuffle.rs` and
  `skip.rs`. A lane index, a length, or an alignment that can be driven out of
  bounds by haystack content is in scope. AVX2 and AVX-512 additionally rest on
  a runtime probe of the *operating system* as well as the silicon — the
  `OSXSAVE` bit and `XCR0`'s promise about the upper half of `ymm`, and for
  AVX-512 the opmask and upper `zmm` state too — so a machine on which that
  probe admits a kernel the OS will not preserve state for is in scope as well.
- **Unbounded work from a pattern.** A pattern is compiled into a DFA before it
  is quotiented. Input that makes construction consume memory or time out of
  proportion to its size is in scope, though see below for what is not.

## What is not a vulnerability

- **A document that survives and does not match.** Passing a non-matching
  document is the contract, not a leak. The filter is an over-approximation, and
  a survivor was never a claim.
- **The gate declining your pattern.** `BuildError::NotWorthIt` is the common
  and intended outcome, and `BuildError::Uncalibrated` on an unmeasured machine
  is fail-closed on purpose. Neither is a denial of service; both mean run your
  matcher unfiltered.
- **A sieve that is slower than the engine on your corpus.** The gate prices
  against measured constants and a shipped prior over a source tree. A corpus
  that is not source code will misprice, which is what `Policy` is for, and a
  bad prediction costs time rather than correctness.
- **Cost proportional to the document.** The kernel reads every byte it is not
  allowed to skip. That is arithmetic.

## What already tries to catch this

None of it is a guarantee, and finding something these missed is exactly the
kind of report we want:

- the differential harness runs **ungated**, on every pattern that harvests a
  quotient rather than the minority the economics admit, because soundness is a
  property of the construction and exists the moment a quotient does;
- dispatch reports the kernel it chose, so a vector-versus-scalar differential
  cannot pass by quietly having tested the scalar path twice;
- every vector path a runner can execute is exercised on real silicon - NEON on
  arm64, and whichever of AVX-512, AVX2 and SSSE3 the machine probes as present
  on x86_64 - against a large battery of mutated haystacks each, natively on all
  six Windows/Linux/macOS × x86_64/arm64 targets
  `.github/workflows/native.yml` covers, never under emulation. Two paths are
  reached by an emulator instead, and both because no native runner can be
  relied on to reach them: `wasm32`'s SIMD128 under `wasmtime`, since a guest
  has no silicon at all, and AVX-512 under Intel SDE, since which x86_64 parts
  the runner fleet allocates is not something a workflow decides. Emulation is
  allowed to prove a kernel correct and is never allowed to price one, so neither
  leg mints anything: SIMD128 has no row at all and is named in `price::DORMANT`
  for that reason, while AVX-512's row came from a Windows runner that has the
  silicon natively, not from the SDE leg that proves it;
- a kernel dispatch would not have chosen is still differentiated, and dispatch
  declining it is now the ordinary case rather than the new-instruction-set case:
  `arch::kernel` elects the kernel `price::MINTED` prices *cheapest*, which on
  x86_64 leaves the widest rung correct, present, measured and unelected -
  `tests/kernels.rs` and the `kernels` fuzz target both sweep
  `shuffle::available()` through the `shuffle::force` seam instead;
- the set classifier is differentiated against a scalar statement of the same
  membership over every byte value, a large battery of pseudo-random sets, and a
  planted escape at every offset of every short length;
- the skip differential asserts a skip lane was actually chosen before it
  compares, and draws haystacks from a narrow alphabet, because uniformly random
  bytes never let a skip loop reach its tail;
- three `cargo-fuzz` targets carry the same properties under a real search
  budget on every push, and a longer monthly campaign whose corpus is carried
  forward from the previous one. `fuzz/skip.rs`
  takes its oracle from the transition rows rather than from `find_scalar`,
  because both searchers read the tables `Skip::of` built and would agree
  exactly if the encoding were wrong. `fuzz/README.md` says what each holds.

## Provenance

[`NOTICE`](NOTICE) records that nothing third-party is bundled here and credits
the published work this crate builds on.
