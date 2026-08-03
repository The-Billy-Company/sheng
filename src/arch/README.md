# `sheng/src/arch/`

Every `unsafe` SIMD intrinsic in this crate lives here, and nowhere else. `shuffle.rs`
and `skip.rs` hold the portable dispatch, the scalar fallback, and the composition
logic; this module holds only the two vector kernels each of them dispatches to.

| module      | what                                                                        |
| ----------- | --------------------------------------------------------------------------- |
| `mod.rs`    | the `Kernel` enum, runtime `kernel()` probe, and the shared `STEP` constant |
| `neon.rs`   | `aarch64` NEON: `sweep_shuffle` (the register kernel) and `classify` (the skip nibble-set test) |
| `ssse3.rs`  | `x86_64` SSSE3: the same two kernels, instruction for instruction           |

`neon` and `ssse3` are each `#[cfg]`-gated to their own architecture, so neither is
ever compiled — let alone linked — on the other target. Every `unsafe fn` here carries
a `# Safety` doc section stating its precondition and a `// SAFETY:` comment at every
`unsafe` block proving that precondition is met, per `undocumented_unsafe_blocks =
"deny"` in `Cargo.toml`. `tests/soundness.rs`'s
`every_accelerated_kernel_agrees_with_the_scalar_reference` runs whichever kernel
dispatch picked on this architecture against thousands of random and adversarial
haystacks per harvested pattern, and asserts a real vector kernel was actually
exercised rather than the scalar path compared against itself — the only thing that
would make an unsafe shortcut here correct is exact agreement with the scalar
specification it shortcuts.
