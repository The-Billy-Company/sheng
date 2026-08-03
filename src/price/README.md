# `sheng/src/price/`

What each kernel costs, and the one inequality that decides whether a sieve arms —
split into the three things that change for different reasons, so re-mint, re-derive,
and re-decide never touch each other's file.

| module            | what                                                                     |
| ----------------- | ------------------------------------------------------------------------ |
| `gate.rs`         | the worth-test arithmetic itself: `survival`, `CostFact`, `NOMINAL_LEN` — pure, no measured numbers |
| `calibration.rs`  | the `Calibration` shape and its per-byte pricing methods, plus `active()` |
| `minted.rs`       | nothing but the measured rows: `MACOS_AARCH64`, `LINUX_X86_64`, `UNMEASURED`, `MINTED` |

`mod.rs` carries the module's own doc (the scale-invariance argument, why nanoseconds
rather than cycles) and re-exports every public name from all three files, so
`price::Calibration`, `price::active`, `price::MINTED`, and the rest resolve exactly
as they did when this was one file — the split changes nothing about the crate's
public surface.

Re-mint with `cargo run --release --example mint`; it prints a `Calibration` literal
ready to paste into `minted.rs`. Never hand-edit a coefficient — a number with no
`host`/`minted` provenance beside it is an anecdote, not a measurement.
