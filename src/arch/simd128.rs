//! WebAssembly SIMD128 kernels: like [`super::neon`] and unlike the x86_64 pair, these
//! need no runtime probe — the caller's `cfg(target_feature = "simd128")` is the whole
//! precondition, because that is the only form the question takes here.
//!
//! # A guest cannot ask what it is running on
//!
//! Every other kernel in this module is selected by asking the machine. WebAssembly has
//! no `CPUID` and no `XCR0`: a module either declares the SIMD proposal in its own
//! bytecode or it does not, and a runtime that cannot execute what it declared refuses
//! the module outright rather than trapping partway through a scan. Feature detection is
//! therefore the *embedder's* job and it happens before any of this code exists, which
//! makes `target_feature = "simd128"` the honest and only spelling — a runtime probe here
//! would be asking a question whose answer was fixed when the `.wasm` was assembled.
//!
//! The consequence worth stating is about the ladder rather than the kernel: on
//! `wasm32` [`super::available`] is decided entirely at compile time, so a build without
//! `simd128` reports `Scalar` and means it, and one with it can never be wrong about
//! having `u8x16_swizzle`.
//!
//! # One slice per register, and the width really is 128 bits
//!
//! `u8x16_swizzle` is a 16-byte shuffle with 16-byte indices and no wider form in the
//! proposal, so this kernel is [`super::ssse3`]'s shape rather than [`super::avx2`]'s:
//! [`shuffle::WAYS`] slices, one to a register, [`super::STEP`] bytes to a step. Out-of-
//! range indices yield zero, exactly as `pshufb` and `vqtbl1q_u8` do, and for the same
//! reason it never matters here — block ids are all below [`LANES`](crate::lattice::LANES).
//!
//! # What a calibration row minted here is a claim about
//!
//! Less than on native silicon, and the row's `host` field is what carries the
//! difference. A `wasm32` row's absolute nanoseconds are a property of the *runtime and
//! the host CPU underneath it*, not of the instruction set — the same module is a
//! different machine under a JIT than under an interpreter. What survives that is the
//! only thing the arming gate reads: the dimensionless ratios between the sieve, the
//! engine's walk, and the engine's skip, all three of which are compiled by the same
//! JIT from the same module in the same run. See [`crate::price`] for why the gate is
//! scale-invariant, and `price::MINTED`'s own documentation for what a row keyed on an
//! architecture does and does not promise.

use core::arch::wasm32::{
    u8x16_bitmask, u8x16_extract_lane, u8x16_max, u8x16_ne, u8x16_splat, u8x16_swizzle, u16x8_shr,
    v128, v128_and, v128_load,
};

use super::STEP;
use crate::lattice::Quotient;
use crate::shuffle::{self, CHUNK, IDENTITY, STRIDE, WAYS};

/// [`crate::shuffle::refutes`]'s composition kernel, instruction for instruction the
/// same as [`super::ssse3::sweep_shuffle`]. See [`super::neon::sweep_shuffle`] for what
/// the register holds and why.
///
/// # Safety
///
/// Requires SIMD128, which is a compile-time property of this module existing at all, so
/// the caller's `cfg` is the whole precondition. The only genuinely unchecked operation
/// is the row load, and a row is exactly [`LANES`](crate::lattice::LANES) bytes indexed
/// by a `u8`.
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn sweep_shuffle(q: &Quotient, hay: &[u8]) -> bool {
    /// The `aarch64` `sweep`, instruction for instruction. See its documentation.
    #[inline(always)]
    unsafe fn sweep(q: &Quotient, block: &[u8], stride: usize, mut live: v128) -> (v128, u8) {
        // SAFETY: every load reads exactly `LANES` bytes from `IDENTITY` or a `q.rows`
        // row, both `[u8; LANES]` arrays, so no load runs past its 16-byte source.
        unsafe {
            let identity = v128_load(IDENTITY.as_ptr().cast::<v128>());
            let mut compose = [identity; WAYS];
            let mut high = [identity; WAYS];
            for step in 0..stride {
                for (way, (f, h)) in compose.iter_mut().zip(&mut high).enumerate() {
                    let byte = block[way * stride + step];
                    let row = v128_load(q.rows[usize::from(byte)].as_ptr().cast::<v128>());
                    *f = u8x16_swizzle(row, *f);
                    *h = u8x16_max(*h, *f);
                }
            }
            // Reading `high[w]` at `live` — before advancing it — is what keeps this
            // exact rather than merely sound.
            let mut seen = live;
            for (f, h) in compose.into_iter().zip(high) {
                seen = u8x16_max(seen, u8x16_swizzle(h, live));
                live = u8x16_swizzle(f, live);
            }
            // Every lane of `seen` is a broadcast maxed with broadcasts, so any lane is
            // the answer and no horizontal reduction is needed.
            (live, u8x16_extract_lane::<0>(seen))
        }
    }

    // Block ids are all below LANES, so no swizzle index is ever out of range and
    // `u8x16_swizzle`'s zeroing behavior is unreachable.
    // SAFETY: `sweep` carries its own proof; `walk` and `u8x16_extract_lane` are safe.
    unsafe {
        // The real trajectory, broadcast — the one value in here that is a state rather
        // than a function.
        let mut live = u8x16_splat(q.start);
        let mut rest = hay;

        while rest.len() >= CHUNK {
            let (chunk, after) = rest.split_at(CHUNK);
            let (next, seen) = sweep(q, chunk, STRIDE, live);
            if seen >= q.threshold {
                return false;
            }
            (live, rest) = (next, after);
        }

        let (paved, trailing) = rest.split_at(rest.len() / WAYS * WAYS);
        let (live, seen) = sweep(q, paved, paved.len() / WAYS, live);
        if seen >= q.threshold {
            return false;
        }
        shuffle::walk(q, trailing, u8x16_extract_lane::<0>(live)).is_some()
    }
}

/// [`crate::skip`]'s nibble-set classifier, one 16-byte step at a time.
///
/// # Safety
///
/// Requires SIMD128 (compile-time, as above). Every load is a full 16-byte read from a
/// `chunks_exact(STEP)` window, and both swizzle indices are masked into `0..16`, so no
/// lane can address outside its table.
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn classify(lo: &[u8; 16], hi: &[u8; 16], hay: &[u8]) -> Option<usize> {
    // SAFETY: every load reads exactly 16 bytes from `lo`/`hi` (`[u8; 16]`) or a
    // `chunks_exact(STEP)` window, so no load runs past its source.
    unsafe {
        let load = |p: *const u8| v128_load(p.cast::<v128>());
        let (lo_tbl, hi_tbl) = (load(lo.as_ptr()), load(hi.as_ptr()));
        let nibble = u8x16_splat(0x0F);
        for (i, block) in hay.chunks_exact(STEP).enumerate() {
            let v = load(block.as_ptr());
            let picked = u8x16_swizzle(lo_tbl, v128_and(v, nibble));
            // `u16x8_shr` shifts 16-bit lanes, so the mask is what makes this a per-byte
            // high nibble rather than a neighbor's bits leaking in. SIMD128 has no
            // 8-bit shift, which is the one place this kernel differs from NEON's.
            let high = v128_and(u16x8_shr(v, 4), nibble);
            let select = u8x16_swizzle(hi_tbl, high);
            // `ne` against zero is the `& != 0` of `member`, and `bitmask` reads the
            // resulting all-ones lanes out one bit apiece — a movemask by another name.
            let hit = u8x16_bitmask(u8x16_ne(v128_and(picked, select), u8x16_splat(0)));
            if hit != 0 {
                return Some(i * STEP + hit.trailing_zeros() as usize);
            }
        }
        crate::skip::wide::tail(lo, hi, hay, STEP)
    }
}
