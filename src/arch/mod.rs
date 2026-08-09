//! Which byte-shuffle instruction set this binary is allowed to use, decided once
//! from `cfg`, a runtime probe, and what [`crate::price`] has actually measured — so
//! [`crate::shuffle`]'s composition kernel and [`crate::skip`]'s nibble classifier can
//! never independently disagree about the target's silicon, and a fourth instruction
//! set, when one arrives, is taught the detection ladder in one place instead of two.
//!
//! The unsafe intrinsics themselves live one level down, one file per instruction
//! set ([`neon`], [`ssse3`], [`avx2`], [`avx512`], [`simd128`]) rather than interleaved
//! with the algorithms that call them: an audit of "what does this crate execute on
//! x86_64" reads three files, not six half-files split across [`crate::shuffle`] and
//! [`crate::skip`].
//!
//! # The crate does not run a kernel it has not priced
//!
//! [`available`] reports every kernel the silicon can execute, fastest first;
//! [`kernel`] returns the fastest one `price::MINTED` holds a row for. That rule is
//! what lets a new instruction set land without a flag day: an unpriced kernel is
//! simply not selected, so adding [`avx2`] cannot move a single arming decision until
//! somebody mints it, and cannot strand an x86_64 machine on
//! [`UNMEASURED`](crate::price::UNMEASURED) by winning a dispatch its calibration has
//! never seen. `examples/mint.rs` reaches the unpriced kernel through [`force`],
//! which is the only way anything does.
//!
//! # "Does this target have a byte shuffle?"
//!
//! That question is spelled out longhand — `any(target_arch = "aarch64",
//! all(target_arch = "x86_64", target_feature = "sse2"), all(target_arch = "wasm32",
//! target_feature = "simd128"))` — rather than hidden behind an alias, because Rust has
//! no `cfg` alias without a build script and this crate would rather repeat a condition
//! than grow one. Teaching the crate a further instruction set means touching three
//! groups of sites, every one of them findable by grepping for `target_feature = "sse2"`
//! and its two siblings above:
//!
//! * **what exists** — a module beside [`ssse3`], its arm of the `x86` probe (or, where
//!   the answer is compile-time, no probe at all), [`STEP`], and its rung of
//!   [`available`];
//! * **what dispatches** — the arms [`kernel`] feeds in [`crate::shuffle::refutes`]
//!   and [`crate::skip::wide::find`];
//! * **what only a vector caller shares** — the composition kernel's shape
//!   ([`WAYS`](crate::shuffle::WAYS), `STRIDE`, `IDENTITY`) and
//!   [`crate::skip::wide::tail`].
//!
//! A wider register is *not* one of those groups, and that is the load-bearing part of
//! the arrangement. [`avx2`] and [`avx512`] carry their own `STEP`, `WAYS` and `STRIDE`
//! beside their intrinsics, because a 32- or 64-byte register buys more slices of the
//! same sixteen-block machine rather than a bigger one — so a caller never has to ask
//! how wide the kernel it dispatched to was, only [`crate::skip::wide::tail`] does, and
//! it is handed the number.

use core::sync::atomic::{AtomicU8, Ordering};

#[cfg(target_arch = "aarch64")]
pub(crate) mod neon;
// `target_feature = "sse2"`, not just `target_arch = "x86_64"`, and the extra
// condition is load-bearing rather than defensive. SSE2 is baseline in the x86_64 ABI
// and on by default everywhere a normal program runs, so this costs nothing — but a
// soft-float target such as `x86_64-unknown-none` turns it off, and there a 128-bit
// vector cannot be held or passed at all, never mind shuffled. Compiling `pshufb`
// there is not merely useless, it fails in the code generator. Such a target reports
// `Kernel::Scalar` from `kernel()` below, which is the truth about what it runs.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
pub(crate) mod avx2;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
pub(crate) mod avx512;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
pub(crate) mod ssse3;
// `simd128` rather than a bare `target_arch`, and for the opposite reason to the x86_64
// pair above: there is no runtime probe to fall back on. A WebAssembly guest cannot ask
// what it is running on — a module declares the SIMD proposal in its own bytecode or it
// does not — so this `cfg` is the whole of the detection, and a build without it reports
// `Kernel::Scalar` and means it.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod simd128;

/// Bytes per 128-bit vector step, which is what `vqtbl1q_u8`/`vld1q_u8`,
/// `pshufb`/`_mm_loadu_si128` and `u8x16_swizzle`/`v128_load` all index. [`avx2`] steps
/// two of these at once and [`avx512`] four, each keeping its own multiplied constant;
/// every caller that needs a step is handed the one its kernel actually used rather than
/// assuming this.
#[cfg(any(
    target_arch = "aarch64",
    all(target_arch = "x86_64", target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(crate) const STEP: usize = 16;

/// The running target's architecture, spelled exactly as `std::env::consts::ARCH`
/// spells it — which is what [`crate::price::MINTED`] keys its rows on and what
/// `examples/mint.rs` writes into a fresh one.
///
/// Read from `cfg` rather than from `std`, because `std::env::consts::ARCH` is a
/// per-target compile-time constant that only *looks* like it needs an operating
/// system to ask. Enumerating it here is what lets the arming gate work on a target
/// with no `std` at all.
///
/// The list covers every architecture with a plausible chance of running a
/// register-resident byte-shuffle kernel. Anything else reads as `"unknown"`, and
/// that catch-all is safe in the one direction that matters: an arch string absent
/// from [`crate::price::MINTED`] resolves to
/// [`UNMEASURED`](crate::price::UNMEASURED), so an unenumerated target declines
/// every pattern rather than inheriting another machine's optimism.
pub const ARCH: &str = if cfg!(target_arch = "aarch64") {
    "aarch64"
} else if cfg!(target_arch = "x86_64") {
    "x86_64"
} else if cfg!(target_arch = "x86") {
    "x86"
} else if cfg!(target_arch = "arm") {
    "arm"
} else if cfg!(target_arch = "riscv64") {
    "riscv64"
} else if cfg!(target_arch = "riscv32") {
    "riscv32"
} else if cfg!(target_arch = "powerpc64") {
    "powerpc64"
} else if cfg!(target_arch = "powerpc") {
    "powerpc"
} else if cfg!(target_arch = "s390x") {
    "s390x"
} else if cfg!(target_arch = "loongarch64") {
    "loongarch64"
} else if cfg!(target_arch = "mips64") {
    "mips64"
} else if cfg!(target_arch = "mips") {
    "mips"
} else if cfg!(target_arch = "sparc64") {
    "sparc64"
} else if cfg!(target_arch = "wasm64") {
    "wasm64"
} else if cfg!(target_arch = "wasm32") {
    "wasm32"
} else {
    "unknown"
};

/// Which byte-shuffle instruction set backs the sieve on this target.
///
/// Reported rather than assumed, for two reasons that are both about not lying.
/// A differential test that compares the vector kernel against the scalar
/// reference proves nothing if dispatch already chose `Scalar` — it would be
/// comparing a function to itself and passing vacuously — so a test has to be able
/// to ask. And [`crate::price`] was measured with a *particular* byte shuffle; a
/// target with a different one, or none, runs a materially different kernel, which
/// the gate must know rather than discover in production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kernel {
    /// `vpshufb` on `zmm` — four 16-lane slices per register, probed at runtime on
    /// `x86_64` against both the silicon and the operating system.
    Avx512,
    /// `vpshufb` — two 16-lane slices per register, probed at runtime on `x86_64`
    /// against both the silicon and the operating system.
    Avx2,
    /// `vqtbl1q_u8` — baseline on every `aarch64`.
    Neon,
    /// `pshufb` — probed at runtime on `x86_64`.
    Ssse3,
    /// `u8x16_swizzle` — decided at compile time on `wasm32`, since a guest has no way
    /// to ask what it is running on.
    Simd128,
    /// No byte shuffle on this target: the reference path is the shipping path.
    Scalar,
}

impl Kernel {
    /// Does this kernel shuffle whole registers of lanes at a time? The economics in
    /// [`crate::price`] assume a kernel that does.
    #[must_use]
    pub const fn is_vector(self) -> bool {
        !matches!(self, Self::Scalar)
    }

    /// `0` is "nothing resolved yet", so a variant's cache code is its discriminant
    /// shifted past it.
    const fn code(self) -> u8 {
        self as u8 + 1
    }

    const fn decode(code: u8) -> Option<Self> {
        Some(match code {
            1 => Self::Avx512,
            2 => Self::Avx2,
            3 => Self::Neon,
            4 => Self::Ssse3,
            5 => Self::Simd128,
            6 => Self::Scalar,
            _ => return None,
        })
    }

    /// Every variant, so a test can sweep the ones this silicon *cannot* run and hold
    /// [`force`] to refusing them — the check that makes every `unsafe` dispatch arm's
    /// "[`kernel`] said so" precondition worth anything.
    ///
    /// Written out rather than derived, and paired with a test that the codes round-trip,
    /// because a variant added above and forgotten here would silently shrink that sweep
    /// to the kernels somebody remembered.
    pub const ALL: &'static [Self] = &[
        Self::Avx512,
        Self::Avx2,
        Self::Neon,
        Self::Ssse3,
        Self::Simd128,
        Self::Scalar,
    ];
}

/// Every kernel this silicon can actually execute, **fastest first** and always ending
/// in [`Kernel::Scalar`], which every target can run.
///
/// Public because it is what `examples/mint.rs` iterates: one mint run on a machine
/// should price every kernel that machine has, not only the one it would have
/// dispatched to. It is also the set [`force`] validates against, which is what keeps
/// forcing a kernel a safe operation rather than a promise.
#[must_use]
pub fn available() -> &'static [Kernel] {
    #[cfg(target_arch = "aarch64")]
    return &[Kernel::Neon, Kernel::Scalar];

    // Every rung this silicon can execute is listed, not only the widest, because a mint
    // prices what it can reach: one run on an AVX-512 box should return four rows, so the
    // ladder below it stays measured and a future decision to *stop* dispatching the
    // widest kernel needs no second machine to make.
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    return if x86::has_avx512() {
        &[Kernel::Avx512, Kernel::Avx2, Kernel::Ssse3, Kernel::Scalar]
    } else if x86::has_avx2() {
        &[Kernel::Avx2, Kernel::Ssse3, Kernel::Scalar]
    } else if x86::has_ssse3() {
        &[Kernel::Ssse3, Kernel::Scalar]
    } else {
        &[Kernel::Scalar]
    };

    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    return &[Kernel::Simd128, Kernel::Scalar];

    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "x86_64", target_feature = "sse2"),
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    &[Kernel::Scalar]
}

/// The resolved answer, memoized — and the one variable [`force`] writes.
///
/// Memoized because [`available`]'s `CPUID` is a serializing instruction costing a
/// couple of hundred cycles: irrelevant once per process, but [`kernel`] is consulted
/// once per *document*, where it would be a measurable tax on a short one. `Relaxed`
/// is sufficient: a race can only have two threads compute the same answer about the
/// same silicon and store it twice.
static RESOLVED: AtomicU8 = AtomicU8::new(0);

/// Which kernel every dispatch point in this crate will actually run here.
///
/// The fastest kernel that is **both** executable on this silicon and priced by
/// [`crate::price::MINTED`] for this [`ARCH`]. Decided once and read by every dispatch
/// point, so what runs and what the crate reports are always the same decision.
///
/// When nothing here is priced at all the answer is the fastest available kernel
/// rather than [`Kernel::Scalar`], and the asymmetry is deliberate: an unpriced
/// machine resolves to [`UNMEASURED`](crate::price::UNMEASURED) and declines every
/// pattern, so there is no arming decision left to protect — and the callers who
/// still run a kernel there ([`Gate::Ungated`](crate::Gate::Ungated), the differential
/// oracles, the mint) have no reason to be slow about it.
#[must_use]
pub fn kernel() -> Kernel {
    if let Some(known) = Kernel::decode(RESOLVED.load(Ordering::Relaxed)) {
        return known;
    }
    let ladder = available();
    let resolved = ladder
        .iter()
        .copied()
        .find(|&kernel| priced(kernel))
        .unwrap_or(ladder[0]);
    RESOLVED.store(resolved.code(), Ordering::Relaxed);
    resolved
}

/// Has anybody timed this (architecture, kernel) pair? The question [`kernel`] asks
/// before it will dispatch to something.
fn priced(kernel: Kernel) -> bool {
    crate::price::MINTED
        .iter()
        .any(|cal| cal.arch == ARCH && cal.kernel == kernel)
}

/// Run every dispatch point in this crate as if `kernel` were what [`kernel`]
/// resolved to — process-wide, for the rest of the process.
///
/// The measurement seam, and the only way to reach a kernel [`crate::price::MINTED`]
/// has no row for: a mint has to be able to time the kernel it is about to price, and
/// a differential test has to be able to compare two kernels on the same silicon
/// rather than whichever one happened to win. Pricing follows, because
/// [`crate::price::active`] reads [`kernel`] — so a forced mint measures a coherent
/// machine rather than one kernel wearing another's row.
///
/// Returns `false`, and changes nothing, for a kernel absent from [`available`]. That
/// check is what makes this safe: every `unsafe` dispatch arm's precondition is
/// "[`kernel`] said so", and [`kernel`] can only say so about silicon the probe
/// confirmed.
///
/// Not what a production caller wants. Dispatch is already the fastest priced kernel,
/// and forcing a slower one only makes the sieve slower while the gate keeps pricing
/// it correctly.
#[must_use]
pub fn force(kernel: Kernel) -> bool {
    let runnable = available().contains(&kernel);
    if runnable {
        RESOLVED.store(kernel.code(), Ordering::Relaxed);
    }
    runnable
}

/// The x86 feature probes, hand-rolled off `CPUID` rather than taken from
/// `std::arch::is_x86_feature_detected!`, so [`kernel`] can answer on a target with no
/// `std` at all.
///
/// `SSSE3` is a plain `CPUID.01H:ECX[9]` bit with **no operating-system
/// participation**: 128-bit register state is in the x86_64 base ABI, so there is
/// nothing an OS could tell us that `CPUID` does not, which makes that probe equal to
/// `std`'s rather than a weaker stand-in for it.
///
/// `AVX2` is the case where that stops being true, and the difference is the whole
/// reason this module reads `XCR0`. A set feature bit only means the silicon *can*;
/// the upper halves of the `ymm` registers are extended state, and whether they
/// survive a context switch is the kernel's promise, not the CPU's. So `OSXSAVE` is
/// consulted for whether `XCR0` is meaningful at all, and then `XCR0` itself for
/// whether the OS actually saves both halves. Reading the feature bit alone is the
/// classic way to turn a supported CPU on an unsupporting kernel into silent register
/// corruption — which for this crate would mean a refutation computed from a
/// half-clobbered composition function.
///
/// `AVX-512` is the same story as `AVX2` with more of the register file at stake, so it
/// is the same two-part question asked of three more `XCR0` bits: the opmask registers
/// and both upper halves of the widened vector state. Nothing about it is a stronger
/// silicon claim than AVX2's — it is a strictly *longer* list of things the operating
/// system has to have promised to save.
///
/// One implementation for both configurations, deliberately. A `cfg` that probed via
/// `std` when it was there and via `CPUID` when it was not would be two answers to a
/// question this module exists to answer exactly once.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod x86 {
    use core::arch::x86_64::{__cpuid, __cpuid_count, _xgetbv};

    /// `CPUID.01H:ECX` bit 9.
    const SSSE3_BIT: u32 = 1 << 9;
    /// `CPUID.01H:ECX` bit 27 — the OS turned `XSAVE` on, so `XGETBV` may be executed
    /// and `XCR0` says something.
    const OSXSAVE_BIT: u32 = 1 << 27;
    /// `CPUID.01H:ECX` bit 28 — the silicon has 256-bit registers.
    const AVX_BIT: u32 = 1 << 28;
    /// `CPUID.07H:EBX` bit 5.
    const AVX2_BIT: u32 = 1 << 5;
    /// `CPUID.07H:EBX` bit 16 — the foundation: 512-bit registers and the opmasks.
    const AVX512F_BIT: u32 = 1 << 16;
    /// `CPUID.07H:EBX` bit 30 — byte and word lanes, which is where `vpshufb` on a
    /// `zmm` and the `vptestmb` beside it actually live. `avx512f` alone would compile
    /// neither, so both bits are the precondition and neither is inferred from the other.
    const AVX512BW_BIT: u32 = 1 << 30;
    /// `XCR0` bits 1 and 2: the OS preserves the SSE state *and* the upper halves of
    /// the `ymm` registers. Both, because saving one without the other leaves exactly
    /// the corruption the pair is checked for.
    const XCR0_YMM: u64 = 0b110;
    /// [`XCR0_YMM`] and bits 5, 6 and 7: the opmask registers, the upper 256 bits of
    /// `zmm0..15`, and the sixteen registers `zmm16..31` that exist only at this width.
    /// All five, for the same reason the pair above is checked together — a kernel that
    /// saved the vector halves but not the opmasks would corrupt exactly the mask
    /// `classify` reads its answer out of.
    const XCR0_ZMM: u64 = XCR0_YMM | 0b1110_0000;

    /// A build told at compile time that it may use an instruction set needs no probe
    /// for it — that is the arm `-C target-cpu=native` takes, and the arm where a
    /// disagreeing probe could only contradict a decision the code generator has
    /// already made everywhere else in the binary.
    pub(super) fn has_ssse3() -> bool {
        cfg!(target_feature = "ssse3") || (leaves() >= 1 && __cpuid(1).ecx & SSSE3_BIT != 0)
    }

    /// Has the operating system promised to preserve `state` across a context switch?
    ///
    /// The half of an `AVX`-family probe that `CPUID` cannot answer, factored out
    /// because both callers below need exactly it and a second copy of this sequence is
    /// a second chance to read `XCR0` without first establishing that `XCR0` means
    /// anything. Short-circuiting is therefore load-bearing rather than stylistic: the
    /// `XGETBV` on the right of the `&&` is only reached once `OSXSAVE` on its left has
    /// said the instruction may be executed at all.
    fn preserved(state: u64) -> bool {
        let base = __cpuid(1).ecx;
        base & (OSXSAVE_BIT | AVX_BIT) == OSXSAVE_BIT | AVX_BIT
        // SAFETY: `_xgetbv` needs `XSAVE`, and `OSXSAVE` just above is precisely the
        // report that the OS has enabled it.
            && unsafe { xcr0() } & state == state
    }

    pub(super) fn has_avx2() -> bool {
        if cfg!(target_feature = "avx2") {
            return true;
        }
        if leaves() < 7 || !preserved(XCR0_YMM) {
            return false;
        }
        __cpuid_count(7, 0).ebx & AVX2_BIT != 0
    }

    /// Both leaf-7 bits and all five `XCR0` bits, because this kernel uses `zmm` lanes
    /// *and* an opmask register and a machine that has one without the other is not one
    /// it can run on.
    pub(super) fn has_avx512() -> bool {
        if cfg!(all(target_feature = "avx512f", target_feature = "avx512bw")) {
            return true;
        }
        if leaves() < 7 || !preserved(XCR0_ZMM) {
            return false;
        }
        let want = AVX512F_BIT | AVX512BW_BIT;
        __cpuid_count(7, 0).ebx & want == want
    }

    /// `CPUID` needs no `unsafe`: the instruction predates the ISA, long mode mandates
    /// it, and `core` therefore exposes it as a safe function on this target. Leaf 0
    /// reports the highest leaf that exists and is always readable, so no later leaf is
    /// queried until leaf 0 has said it is there.
    fn leaves() -> u32 {
        __cpuid(0).eax
    }

    /// # Safety
    ///
    /// Requires `XSAVE`, which is what `OSXSAVE` in [`has_avx2`] reports. Leaf 0 of the
    /// extended-state mask is architecturally defined whenever it is.
    #[target_feature(enable = "xsave")]
    unsafe fn xcr0() -> u64 {
        // SAFETY: `xsave` is a precondition of calling this function at all.
        unsafe { _xgetbv(0) }
    }
}
