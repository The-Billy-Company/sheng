Two more kernels, a list that will not let one sleep quietly, and a mint that can finish.

`src/arch/` gains AVX-512 for x86_64 and SIMD128 for `wasm32`, which puts the ladder at
four rungs on x86_64 and two on WebAssembly. Both are the same two kernels the other
backends implement — the composition sweep and the skip classifier — and both agree with
`shuffle::scalar` byte for byte, which is the only thing that could make either correct.

AVX-512 is 64 bytes per `vpshufb`, and for this crate that is *four* slices per register
rather than one wider slice, for the reason AVX2 gets two: `vpshufb` is defined per
128-bit lane at every width it exists at, and the sieve's step is a 16-entry table lookup,
so a `zmm` carries four independent trajectories rather than one four-times-wider one.
`vpermb` is deliberately declined — a cross-lane permute buys a wider table, and the table
is `LANES` entries because that is a quotient's block count and not because sixteen is what
a register held. The classifier does get something the narrower kernels cannot have:
`vptestmb` writes "these lanes ANDed nonzero" straight into a mask register, which is
exactly the membership question, where SSSE3 and NEON must compare against zero and invert.

The probe grew with it. AVX-512 needs `avx512f` and `avx512bw` from `CPUID` and three
`XCR0` bits — opmask, and both halves of the upper `zmm` state — because a width whose
registers the operating system will not preserve across a context switch is not a width the
process has, and `CPUID` alone cannot say. Same shape as the `ymm` check AVX2 already
carried, one register wider.

AVX-512 also gets a CI leg of its own, under Intel SDE, because it is the one rung whose
correctness no runner here can be relied on to prove — the native legs sweep whatever the
probe admitted, and which x86_64 parts the fleet allocates is not something a workflow
decides. The leg fails loudly if `arch::available` does not come back holding `Avx512`,
since a leg that quietly re-tested SSSE3 under an emulator would be worse than no leg.
Emulation is allowed to prove a kernel correct and is never allowed to price one:
`price::MINTED` takes rows from real silicon and nowhere else.

`wasm32` is the first target whose kernel is chosen at compile time rather than probed: a
guest has no `CPUID`, so `-C target-feature=+simd128` decides, and `arch::available` reads
`cfg` where it reads a register everywhere else. A new CI leg builds both halves and runs
the differential under `wasmtime`, handing the corpus in through a WASI preopen — the
SIMD128 kernel had compiled on nothing and executed nowhere before that leg existed.

`price::DORMANT` is the new list, and it is the answer to a failure this crate had already
built the machinery to avoid and no way to notice. Dispatch declines a kernel `MINTED` has
no row for, which makes shipping an unmeasured kernel safe — but a kernel nobody has minted
yet and a kernel that *stopped* being priced are indistinguishable in `MINTED`: both are
simply absent. The first is a plan and the second costs every machine on that silicon its
throughput while every test still passes, because the narrower kernel is still correct.
`DORMANT` names the first with its reason, and a test holds the two lists to each other in
both directions — so a kernel left out fails the build, and a row landed later fails the
build until its line here is deleted.

Which leaves the mint, whose x86_64 leg is what all three of those kernels are waiting on,
and which could not have completed if anyone had dispatched it. A price row is nanoseconds
per byte read from *memory*, so `examples/mint.rs` refuses a corpus under 32 MiB — and the
workflow's corpus was ~14 MiB, sized for `survey`'s 8 MiB prior floor. Worse, the refusal
sat after the persistence sweep and the workflow's own size gate sat after the whole run, so
the failure mode was most of an hour of a runner's time to report a precondition. Four
pinned upstream standard libraries (Zig, TypeScript, swift-syntax, and two of Go's
packages) bring the corpus to ~60 MiB across five language families, and `mint -- corpus`
applies the same floor by walking the same tree with the same code, in seconds, before the
measurement rather than during it.

`native.yml` is now called from `ci.yml` as well as `release.yml`, which is what its own
header has always said and what `README.md` has always claimed: the six-cell cross-OS proof
on real, never-emulated silicon ran on tags only. And no CI leg is named for a kernel any
more. `arch::available` probes at runtime, so which rungs an `x86_64` runner offers is a
fact about the machine the fleet allocated — a leg labelled `x86_64-ssse3` was a guess about
someone else's hardware written into a job name. The legs print the ladder they were handed
instead.
