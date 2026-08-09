# `fuzz/` — the soundness oracles under a coverage-guided fuzzer

Three targets, one property between them: **a refutation is never wrong.** That
is the crate's entire risk surface, and it is a property with no smaller signal
attached — a sieve that refutes a document it should not produces no crash, no
panic and no wrong-looking output. It drops a match, silently, forever. So the
search budget *is* the assurance, which is why there is a campaign here and not
only a smoke.

| target | what it holds, and against what |
| ------------ | ----------------------------------------------------------------------- |
| `soundness` | matched implies not refuted, against `regex-automata` on the same pattern |
| `skip` | the skip classifier, against the transition rows it summarizes |
| `kernels` | every kernel the silicon has, against the scalar reference |

Patterns are **not** fuzzed. A random string is regex noise far more often than
it is a buildable automaton, so a syntax-fuzzing target would spend nearly its
whole budget on `regex-automata` rejections instead of on haystacks — the
dimension actually worth exploring. `soundness` and `kernels` take a pattern
index in their first byte and select from the same fixed, always-valid slate
`tests/soundness.rs` sweeps; everything after it is the haystack.

## What each target adds that the others cannot

**`soundness`** is `tests/soundness.rs` handed a real search budget: build the
sieve ungated, ask `regex-automata` whether the pattern matches, and assert the
sieve never refuted a haystack that did. Ungated deliberately — soundness is a
property of every quotient the lattice harvests, not of the minority the cost
gate admits.

**`skip`** exists because the skip loop is the one stage where being fast and
being wrong look identical from the outside. Its oracle is not `find_scalar`:
both searchers read the same two nibble tables `Skip::of` built, so the pair
agrees exactly when the *encoding* is wrong. The assertion instead reads the
answer off the transition rows — the first byte whose row moves the run out of
its block — which puts `Skip::of` under test rather than assuming it, in both
directions. A set the instrument represents exactly must be accepted, and a
wide set with a byte at or above `0x80` must be refused, because the eight
high-nibble bits are spent and a ninth would alias onto a member.

**`kernels`** exists because dispatch is deliberately conservative:
`arch::kernel()` returns the fastest kernel `price::MINTED` has a *row* for, so
the newest instruction set is exactly the one `soundness` cannot reach until
somebody mints it. `shuffle::force` closes that gap and refuses any kernel the
runtime probe did not admit, so nothing here can execute an instruction the host
lacks. On arm64 this sweeps one kernel, since NEON is baseline; on x86_64 it
sweeps `Avx2`, `Ssse3` and `Scalar`.

This is a standalone `cargo-fuzz` crate (nightly-only — libFuzzer needs
sanitizer support the stable toolchain does not ship) excluded from the parent
workspace so its lockfile never constrains the library it targets.

## Running it

```bash
./seeds.sh                                              # real bytes to start from
cargo +nightly fuzz run skip                            # until Ctrl-C or a crash
cargo +nightly fuzz run skip -- -max_total_time=90      # bounded, what a PR runs
cargo +nightly fuzz build                               # compile without running
cargo +nightly fuzz cmin skip                           # shrink the corpus
```

`seeds.sh` is worth the one command. `corpus/` is machine-local and gitignored,
so a fresh clone starts every target cold, and the first thousands of executions
then go on rediscovering that a haystack wants to be text and wants to be long
enough to cross `shuffle::CHUNK` — facts about the input shape rather than
anything a search should have to find. The seeds are cut from this repository's
own source, on the same reasoning `examples/common.rs` walks a real tree: a
synthetic haystack answers every question with whatever generated it.

A crash writes its input to `artifacts/<target>/` and prints the bytes that broke
the property — reproduce with:

```bash
cargo +nightly fuzz run skip artifacts/skip/<crash-file>
cargo +nightly fuzz tmin skip artifacts/skip/<crash-file>   # shrink it first
```

## What CI runs

`.github/workflows/fuzz.yml` has two jobs, and they differ in kind rather than
in length.

The **smoke** is 90 seconds per target on every push and pull request, seeded.
It is a tripwire: enough to catch a regression a fixed differential sweep would
miss, not enough to establish anything.

The **campaign** is monthly, two hours per target by default, and it carries its
corpus forward — so each run starts from everything the previous ones found
interesting. That hand-off is the whole difference between a campaign and the
same first minutes of search repeated twelve times a year, and it is why the
corpus travels in an **artifact rather than a cache**: a cache entry is evicted
after seven days without a read, and the gap between two monthly runs is four
weeks, so a cached corpus would be reliably gone every time.

Continuous coverage beyond that is OSS-Fuzz's job; the build recipe lives in
`oss-fuzz/`, which explains what has to be true before submitting it.

`corpus/`, `artifacts/`, and `coverage/` are machine-local and gitignored;
nothing under this directory needs to be committed except the targets, the
manifest, and `seeds.sh`.
