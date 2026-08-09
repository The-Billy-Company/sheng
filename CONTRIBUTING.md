# Contributing

Thanks for looking. This is a small crate with one unusual property, and
everything below follows from it: `sheng` decides that a document cannot match,
and a caller then never looks at that document. A bug here is silent by
construction.

So the bar for a change is not "the tests are green". It is "what would have
caught this if it had been wrong", and reviews ask that question first.

## What This Repository Is, and What It Is Not

It is one library crate. There is no binary, no CLI, and no plugin surface - a
caller builds a `Sieve` and asks it `refutes`.

It is a prefilter, so it never answers where a match is or whether one exists.
That belongs to [regex-automata](https://github.com/rust-lang/regex), which this
crate reads its automaton from and calls to confirm every survivor.

It is a Rust port of one rung of our Zig engine
[irregex](https://github.com/The-Billy-Company/irregex), not a second design. A
change to the SP-quotient harvest or the register kernel usually wants to land
in both, and a change to what a pattern means belongs to neither of us.

## Setup

You need a Rust toolchain and nothing else - `rust-toolchain.toml` pins 1.96.0,
so `rustup` will fetch it on first build:

```bash
git clone https://github.com/The-Billy-Company/sheng
cd sheng
cargo test --release
```

Run the suite in release. The differential harness drives 30 000 mutated
haystacks through both a vector kernel and a scalar reference, and a debug build
turns a two-second gate into a coffee break.

## The Test Loop

Four examples double as instruments, and each answers a different question:

```bash
cargo run --release --example survey   # end to end; asserts no armed row loses
cargo run --release --example skip     # per-lane skip-vs-compose, audits the planner
cargo run --release --example bench    # per-stage build cost and kernel ns/byte
cargo run --release --example mint     # re-mint the prior and the calibration
```

`survey` is a gate rather than a report: it fails if any pattern the model armed
came out below 1.000x, so a coefficient that drifts generous fails loudly
instead of quietly costing every caller a few percent. Run it before and after
anything that touches `src/price/`.

Give it a real corpus. Each row is five samples of a min-of-five and is only
called a loss when its whole interval sits below 1.000x, and below 8 MiB the
survey declines to judge at all - a cache-resident corpus lets the engine's own
accelerator run at tens of gigabytes a second, which no per-byte calibration
describes. `SHENG_CORPUS=/path/to/a/big/tree` is how you get a verdict; against
this repository alone you will correctly get a refusal.

## The Constraints A Change Is Held To

Soundness is not negotiable and is not a test-suite property. A partition that
is not closed under the transition function, a classifier that approximates a
set it should have refused, and a collapse that reads the wrong lane are all the
same bug - a document refuted that holds a match - and none of them announce
themselves.

Test every pattern that harvests a quotient, not the ones the gate admits.
`Gate::Ungated` exists for exactly this: the economics decide what ships, and
soundness has to hold on everything the construction can build.

Keep the scalar reference honest. Every vector path is differentiated against a
plain statement of the same definition, and `shuffle::kernel()` reports which
kernel actually ran so a differential cannot pass by testing the scalar path
twice. If you add a vector path, add its reference in the same commit.

Do not weaken an assertion to go green. A failing test is a report that the
implementation is wrong until you have proven otherwise, and a frozen artifact -
a calibration constant, a benchmark baseline - is a test wearing a different
hat.

Measure before you claim. Numbers in the README and in changelog fragments were
taken back to back on one idle machine against one corpus, because absolute
figures here drift 25% under load. Quote a ratio, name the machine, and say what
moved.

## Unsafe Code

`shuffle.rs` and `skip.rs` dispatch into hand-written NEON, AVX2 and SSSE3, which
live one file per instruction set in `src/arch/`. New `unsafe` is accepted, and it
carries three obligations in the same PR:

1. **A scalar reference** stating the same thing in safe Rust, which the tests
   compare against rather than reimplementing the vector logic.
2. **Adverse cases, not mirrors** - every byte value against every set shape,
   randomized inputs from a seeded generator, and a planted target at every
   offset of every length. The tail of the last chunk is where these break.
3. **A refusal path** - if the fast path cannot represent an input exactly it
   returns `None` and the caller falls back. Approximating is the one
   unrecoverable bug in this crate.

A whole new kernel carries a fourth, and it is the one that surprises people: it
will not run. `arch::kernel` returns the fastest kernel `price::MINTED` holds a row
for, so a kernel with no row is inert by construction rather than by oversight -
running it would price arming with some other kernel's nanoseconds. Dispatch wakes it
only once somebody runs `.github/workflows/mint.yml` on that hardware and pastes the
rows in. Until then `tests/kernels.rs`, the `kernels` fuzz target, and
`shuffle::force` are what exercise it.

## Every Change Carries Its Own News

Write a towncrier fragment in the same PR:

```bash
# types: added changed deprecated removed fixed security
towncrier create '+<slug>.<type>.md'
```

Fragment names read like the sentence they are:
`+the-cheapest-byte-is-the-one-you-never-load.added.md`. The leading `+` tells
towncrier there is no issue number attached. The body is prose for a person
reading release notes - what changed and what it means for them, not a
restatement of the diff.

Skip it only for comment-only, format-only, or genuinely invisible internal
work. When unsure, write it.

This repository's tag, changelog, and publish steps are one instance of a
model shared across every Billy-Company OSS package - see
[RELEASING.md](https://github.com/The-Billy-Company/.github/blob/main/RELEASING.md)
for the lifecycle this feeds into and why it's shaped this way.

## Commits and Pull Requests

Commit subjects are a conventional prefix plus a lowercase sentence that says
what changed, in the voice of the change rather than the ticket:

```text
feat: the cheapest byte is the one you never load
fix: a settled-looking chain still had mass to find
perf: the kernel stopped waiting for itself
```

Prefixes in use: `feat` `fix` `perf` `refactor` `docs` `test` `build` `ci`
`chore`. Keep the subject under about 72 characters and put the reasoning in the
body, where reviewers and `git log` both find it.

The subject line becomes the squash commit message, and that is what
release-please reads to pick the next version - shifted one column left while
this crate is still 0.x. A breaking change and a `feat` both take the minor,
so `0.1.0` goes to `0.2.0` for either and to `0.1.1` for everything else, and
nothing declares 1.0.0 on our behalf until we mean to. The post-1.0 table, and
the `Release-As: X.Y.Z` footer that pins an exact version the rules would not
pick, are in the org standard, [What Picks the
Number](https://github.com/The-Billy-Company/.github/blob/main/RELEASING.md#what-picks-the-number).

For the pull request: one concern per PR, and describe what would have caught
the bug if it had existed. Reviews here ask three questions more than any
others - what proves this, what does it cost, and what did it replace.
Answering them in the description saves a round trip.

If you removed something a newer path superseded, remove it completely. Leaving
the old implementation beside the new one to be safe is how a codebase grows two
spellings of the same bug.

## Calibration Is Per Machine, Not Per Contributor

`price::MINTED` holds one row per (architecture, kernel) pair somebody has
measured, and a machine with no row declines every pattern rather than
inheriting another machine's silicon. That refusal is correct, so do not add a
row by copying a neighboring one.

To add yours, run `cargo run --release --example mint` on an idle machine and
paste what it emits. Every constant it prints carries the machine, the kernel,
and the date that produced it; a measured value with no machine beside it is an
anecdote.

## Licensing

This project is Apache-2.0. There is no CLA: contributions are accepted under
the same license the project already carries, per the inbound=outbound norm in
section 5 of the license itself.

Nothing third-party is bundled here. [`NOTICE`](NOTICE) says so, and credits the
published work the design builds on. If you bring in code, data, or an idea from
another tool, credit it at the call site and in the NOTICE.

## Reporting A Vulnerability

Not here. [`SECURITY.md`](SECURITY.md) has the private channel, and a false
refutation belongs in it rather than in a public issue.
