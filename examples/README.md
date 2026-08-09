# `sheng/examples/`

Four programs and one shared module, and none of the programs is a demo. Each reads
**real source**, because a synthetic corpus would answer its question with whatever
generator wrote it — and each finds it by climbing to the enclosing checkout, so it
runs from anywhere. `$SHENG_CORPUS` points them at the bytes you actually search,
which is the whole reason to re-mint.

`common.rs` holds that discovery, the corpus walk, and the machine/date stamp — one copy,
because two copies of a walker rooted at `"."` is how a measurement comes to depend on
the directory you happened to be standing in. It is a module rather than a third example,
which is why `Cargo.toml` declares its example targets instead of letting Cargo discover
them.

## `mint` — where the constants come from

```bash
cargo run --release --example mint
```

Measures everything the crate cannot derive, and prints it as Rust ready to paste into
`src/prior/minted.rs` and `src/price/minted.rs`:

- the **first-order byte-class chain** and the per-byte marginals (`SOURCE`, `SOURCE_BYTES`);
- **nanoseconds per byte** for the sieve kernel and for the rival engine both with and
  without its start-state accelerator;
- the **excursion coefficient**, solved by inverting the accelerated blend across eleven
  lead bytes spanning two orders of magnitude of frequency, and reported with its spread
  so a coefficient that cannot carry the model says so.

A run measures a corpus or a machine, never both. Named, it mints that one prior and
stops — a price row swept over prose would still be keyed on (architecture, kernel), so
pasting one would overwrite a row measured on the bytes its callers actually search:

```bash
SHENG_CORPUS=/tmp/gutenberg SHENG_KINDS=txt \
    cargo run --release --example mint -- prose
```

`$SHENG_KINDS` is required for that form rather than decorative: a tree of `.txt` is
invisible to the source-extension default however `$SHENG_CORPUS` is aimed, which is the
whole reason every shipped prior once described a code tree. The three non-source priors
are minted from corpora pinned by commit in `.github/workflows/priors.yml`, which
re-derives them on every relevant push and fails if a cell has drifted — a prior is a
count, so unlike a price row it can be checked rather than merely stamped.

Every block is stamped with the architecture, the kernel dispatch chose, the host and the
date — from `std` alone, no `uname` shell-out. A measured value with no machine beside it
is an anecdote, and a price row measured through `pshufb` is not a price for a machine
without it. The price row prints named for its target (`LINUX_X86_64_SSSE3`), because it belongs
in `price::MINTED` rather than replacing a global default.

## `survey` — the gate that judges the gate

```bash
cargo run --release --example survey
```

Times each pattern twice over a large set of real documents — the engine alone, then
the engine behind its sieve — and prints the arming decision beside the measured
ratio, including the full arithmetic for every pattern that declined.

**It asserts.** A row the model armed must come out above unity, and a slate where
nothing arms fails too. That makes it the standing check on whichever `price::MINTED` row
this machine resolves to: a coefficient that drifts generous fails loudly here instead of
quietly costing every caller. Re-run it after any re-mint — and on a machine with no
row at all, expect the honest version of failure: a decline for every pattern, each
naming the measurement nobody took.

**It also refuses.** Every timing is several samples of a min-of-several, so each row
carries an interval rather than a number, and a row is only called a loss when the whole
interval sits below unity. A row whose interval straddles is reported undecided and
asserts nothing, because the alternative is a gate that fires on the clock's own
resolution — against a tiny tree, rows can read as losses that a real corpus prices as
wins.

A small working set declines to judge the model at all. That is not a noise threshold: a
calibration is nanoseconds per byte read from memory, and a corpus small enough to sit in
cache never reads from memory, so the engine's `memchr` accelerator runs at memory
bandwidth and beats every price the crate knows for reasons that have nothing to do with
the sieve. Point `$SHENG_CORPUS` at a tree with real volume.

## `bench` — isolating where the time goes

```bash
cargo run --release --example bench
```

`survey` answers "is arming worth it", which conflates two things: a faster kernel
changes *which* rows arm, so its geomean moves for two reasons at once. `bench` times
the two halves apart instead — nanoseconds per byte for the kernel itself, per document
size, and microseconds for building a sieve — ungated, so a pattern the economics would
decline still gets measured.

## `skip` — is there a byte-skip worth building

```bash
cargo run --release --example skip
```

Before a line of skip kernel gets written, this reports the three numbers that decide
whether it would pay: the escape width leaving the quotient's start block, how much of
the real corpus actually sits in that block, and whether the rival engine's own
accelerator already covers the same ground.
