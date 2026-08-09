# `sheng/src/price/`

What each kernel costs, and the one inequality that decides whether a sieve arms —
split into the three things that change for different reasons, so re-mint, re-derive,
and re-decide never touch each other's file.

| module            | what                                                                     |
| ----------------- | ------------------------------------------------------------------------ |
| `gate.rs`         | the worth-test arithmetic itself: `survival`, `CostFact`, `NOMINAL_LEN` — pure, no measured numbers |
| `calibration.rs`  | the `Calibration` shape and its per-byte pricing methods, plus `active()` |
| `minted.rs`       | nothing but the measured rows — `MACOS_AARCH64_NEON`, `LINUX_X86_64_SSSE3`, `UNMEASURED`, `MINTED` — and `DORMANT`, which names the kernels they do not price |

`mod.rs` carries the module's own doc (the scale-invariance argument, why nanoseconds
rather than cycles) and re-exports every public name from all three files, so
`price::Calibration`, `price::active`, `price::MINTED`, and the rest resolve exactly
as they did when this was one file — the split changes nothing about the crate's
public surface.

Re-mint with `cargo run --release --example mint`; it prints a `Calibration` literal
ready to paste into `minted.rs`. Never hand-edit a coefficient — a number with no
`host`/`minted` provenance beside it is an anecdote, not a measurement.

One run prints a row for every kernel the silicon can execute, not just the one
dispatch elected, because dispatch will not elect a kernel this file has no row
for — so a mint that followed dispatch could never reach a newly added
instruction set. Pasting a row in is therefore what wakes a kernel, and it comes
with a deletion: `DORMANT` names the same kernel and its reason, and `minted.rs`'s
own tests hold the two lists to each other in both directions.

A row is keyed on `(os, arch, kernel)`, so it takes one run **per machine** — six, to
cover what `.github/workflows/native.yml` proves correct. That column is not
fastidiousness: with the key two columns wide, three of those six legs priced themselves
from a fourth machine's row and `examples/survey.rs` caught every one of them arming a
pattern that then lost. And among the rows a machine does have, dispatch picks the
cheapest `sieve`, never the widest register — on x86_64 those are different kernels.
