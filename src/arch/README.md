# `sheng/src/arch/`

Every `unsafe` SIMD intrinsic in this crate lives here, and nowhere else. `shuffle.rs`
and `skip.rs` hold the portable dispatch, the scalar fallback, and the composition
logic; this module holds only the vector kernels each of them dispatches to.

| module       | what                                                                        |
| ------------ | --------------------------------------------------------------------------- |
| `mod.rs`     | the `Kernel` enum, runtime `kernel()` probe, and the shared `STEP` constant |
| `neon.rs`    | `aarch64` NEON: `sweep_shuffle` (the register kernel) and `classify` (the skip nibble-set test) |
| `avx512.rs`  | `x86_64` AVX-512: the same two kernels at 64 bytes a shuffle — four slices per register, and a mask register in place of the movemask |
| `avx2.rs`    | `x86_64` AVX2: the same two kernels at 32 bytes a shuffle — two slices per register, since `vpshufb` is per-128-bit-lane |
| `ssse3.rs`   | `x86_64` SSSE3: the same two kernels, instruction for instruction           |
| `simd128.rs` | `wasm32` SIMD128: the same two kernels at 16 bytes, `u8x16_swizzle` for the shuffle |

Each is `#[cfg]`-gated to its own architecture, so an `aarch64` build never compiles —
let alone links — the x86_64 bodies, or the reverse. Every `unsafe fn` here carries
a `# Safety` doc section stating its precondition and a `// SAFETY:` comment at every
`unsafe` block proving that precondition is met, per `undocumented_unsafe_blocks =
"deny"` in `Cargo.toml`. `tests/soundness.rs`'s
`every_accelerated_kernel_agrees_with_the_scalar_reference` runs whichever kernel
dispatch picked on this architecture against thousands of random and adversarial
haystacks per harvested pattern, and asserts a real vector kernel was actually
exercised rather than the scalar path compared against itself — the only thing that
would make an unsafe shortcut here correct is exact agreement with the scalar
specification it shortcuts.

Which leaves one gap that test cannot close, because `kernel()` is deliberately
conservative: it returns the kernel `price::MINTED` prices *cheapest*, so a kernel that
compiles and runs but has not been minted is never what dispatch picks — and neither is one
that was minted and measured slower, which on x86_64 is the widest rung here.
`tests/kernels.rs` and the `kernels` fuzz target close it from the other side, sweeping
every entry `shuffle::available()` reports through the `shuffle::force` seam — which
refuses any kernel this same runtime probe did not admit, so forcing can select an
unpriced kernel and still cannot select an absent one.

AVX2 is also the one kernel whose precondition is not only about the silicon. `vpshufb`
needs the operating system to preserve the upper half of `ymm` across a context switch,
which `CPUID` cannot answer — so the probe in `mod.rs` additionally requires `OSXSAVE` and
reads `XCR0` through `xgetbv` for the OS's own promise. A `CPUID`-only probe would admit
AVX2 on a kernel that silently discards those bytes.
