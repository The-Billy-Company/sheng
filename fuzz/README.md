# `fuzz/` — the soundness oracle under a coverage-guided fuzzer

One target, one property: **matched implies not refuted.** This is the exact
invariant `tests/soundness.rs` proves over a fixed sweep of real and mutated
bytes; `fuzz_targets/soundness.rs` hands the same oracle to `cargo-fuzz` so it
can spend a real search budget hunting for a byte shape nobody thought to
hand-write.

Patterns are **not** fuzzed. A random string is regex noise far more often
than it is a buildable automaton, so a syntax-fuzzing target would spend
nearly its whole budget on `regex-automata` rejections instead of on haystacks
— the dimension actually worth exploring. Instead the first input byte
selects one of the same fixed, always-valid patterns `tests/soundness.rs`
sweeps, and every byte after it is the haystack `Sieve::refutes` is asked to
judge, checked against `regex-automata`'s own matcher on the identical
pattern. A second assertion holds the vector kernel to the scalar reference
(`Sieve::refutes_scalar`) on every input, catching kernel disagreements the
fixed differential sweep would not think to try.

This is a standalone `cargo-fuzz` crate (nightly-only — libFuzzer needs
sanitizer support the stable toolchain does not ship) excluded from the parent
workspace so its lockfile never constrains the library it targets.

## Running it

```bash
cargo +nightly fuzz run soundness # until Ctrl-C or a crash
cargo +nightly fuzz run soundness -- -max_total_time=60 # bounded smoke, what CI runs
cargo +nightly fuzz build # compile without running
```

A crash writes its input to `artifacts/soundness/` and prints the pattern
index and haystack bytes that broke soundness — reproduce with:

```bash
cargo +nightly fuzz run soundness artifacts/soundness/<crash-file>
```

`corpus/`, `artifacts/`, and `coverage/` are machine-local and gitignored;
nothing under this directory needs to be committed except the target itself.
