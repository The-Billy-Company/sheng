The AVX2 and AVX-512 composition kernels kept their four chains on the stack instead of in
registers. Both wrote `for (reg, (f, h)) in compose.iter_mut().zip(&mut high).enumerate()`,
and in a release build that zip did not inline — `Zip::new` survives as a call — so LLVM
declined to unroll the four-way loop, which made `[__m512i; 4]` and `[__m256i; 4]`
addressable and spent a full-width spill and reload on every chain every step:

```asm
vpshufb   (%rax,%rbx), %zmm0, %zmm0   ; compose[reg], loaded from the stack
vmovdqa64 %zmm0, (%rax,%rbx)          ; and stored straight back
vpmaxub   (%rax,%rbx), %zmm0, %zmm0   ; high[reg], likewise
vmovdqa64 %zmm0, (%rax,%rbx)
```

The op count is not the injury. `compose[reg]` is a *recurrence* — each step composes onto
the previous one — so a spill puts store-to-load forwarding on the one dependency the
kernel is built to keep inside a register, on a target with thirty-two `zmm` registers and
eight of them wanted. AVX-512 additionally called out to `core::array::from_fn`'s `FnMut`
machinery eight times per hot loop to build a `[*const u8; 4]` of row pointers.

Both now index a constant trip count, and `quad` takes four pointers the way AVX2's `pair`
always took two. Measured in the emitted assembly of a real release build, per kernel:
spill stores 28 to 7, and the wide `vpshufb` count — the tell for whether the four-chain
loop unrolled at all — from 2 to 8.

This was found while asking why AVX-512 measured *slower* than AVX2, 0.335 against
0.290 ns/B, despite consuming four times the bytes per step — and it is worth recording that
it turned out not to be the answer. Both kernels got faster once their chains stayed in
registers, and the ordering between them did not change: re-minted on real silicon they read
0.458 against 0.376 ns/B, the same ranking with a wider gap. So this is a fixed bug and a
ruled-out hypothesis, not an explanation; what remains unexplained is left unexplained in
`price::WINDOWS_X86_64_AVX512` rather than guessed at. `ssse3`, `neon` and `simd128` were
never affected and are unchanged — one row per register is an indexed read LLVM already
folds, which is exactly why the fault looked like a property of AVX-512 instead of a
property of how the loop was written.

Nothing about what these kernels compute has changed: same indices, same order, same
operations, so `tests/kernels.rs` holds them to the scalar reference as before — on native
silicon for AVX2 and under Intel SDE for AVX-512.
