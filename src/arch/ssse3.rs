//! SSSE3 kernels: probed at runtime by [`super::kernel`] before either of these is
//! ever called.

use std::arch::x86_64::{
    __m128i, _mm_and_si128, _mm_cmpeq_epi8, _mm_cvtsi128_si32, _mm_loadu_si128, _mm_max_epu8,
    _mm_movemask_epi8, _mm_set1_epi8, _mm_setzero_si128, _mm_shuffle_epi8, _mm_srli_epi16,
};

use super::STEP;
use crate::lattice::Quotient;
use crate::shuffle::{self, CHUNK, IDENTITY, STRIDE, WAYS};

/// Low lane of a vector every lane of which is already the same byte.
unsafe fn low(v: __m128i) -> u8 {
    // SAFETY: `_mm_cvtsi128_si32` reads the low 32 bits of any `__m128i`; it has no
    // alignment or initialization precondition beyond a valid vector, which every
    // caller here already holds.
    unsafe { _mm_cvtsi128_si32(v) as u32 as u8 }
}

/// [`crate::shuffle::refutes`]'s composition kernel, instruction for instruction
/// the same as [`super::neon::sweep_shuffle`]. See that module's documentation for
/// what the register holds and why.
///
/// # Safety
///
/// Requires SSSE3, which [`crate::shuffle::refutes`] probes for before dispatching
/// here. The only genuinely unchecked operation is the row load, and a row is
/// exactly [`LANES`](crate::lattice::LANES) bytes indexed by a `u8`.
#[target_feature(enable = "ssse3")]
pub(crate) unsafe fn sweep_shuffle(q: &Quotient, hay: &[u8]) -> bool {
    /// The `aarch64` `sweep`, instruction for instruction. See its documentation.
    #[inline(always)]
    unsafe fn sweep(q: &Quotient, block: &[u8], stride: usize, mut live: __m128i) -> (__m128i, u8) {
        // SAFETY: caller requires SSSE3 (this fn is only called from
        // `sweep_shuffle`, itself gated on `#[target_feature(enable = "ssse3")]`).
        // Every load reads exactly `LANES` bytes from `IDENTITY` or a `q.rows` row,
        // both `[u8; LANES]` arrays, so no load runs past its 16-byte source.
        unsafe {
            let identity = _mm_loadu_si128(IDENTITY.as_ptr().cast::<__m128i>());
            let mut compose = [identity; WAYS];
            let mut high = [identity; WAYS];
            for step in 0..stride {
                for (way, (f, h)) in compose.iter_mut().zip(&mut high).enumerate() {
                    let byte = block[way * stride + step];
                    let row = _mm_loadu_si128(q.rows[usize::from(byte)].as_ptr().cast::<__m128i>());
                    *f = _mm_shuffle_epi8(row, *f);
                    *h = _mm_max_epu8(*h, *f);
                }
            }
            let mut seen = live;
            for (f, h) in compose.into_iter().zip(high) {
                seen = _mm_max_epu8(seen, _mm_shuffle_epi8(h, live));
                live = _mm_shuffle_epi8(f, live);
            }
            (live, low(seen))
        }
    }

    // Block ids are all below LANES, so no shuffle index ever has its high bit set
    // and `pshufb`'s zeroing behavior is unreachable.
    // SAFETY: caller requires SSSE3, which `#[target_feature(enable = "ssse3")]`
    // above makes a precondition of calling this function at all. `sweep` carries
    // its own proof; `low` and `walk` are safe.
    unsafe {
        let mut live = _mm_set1_epi8(q.start as i8);
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
        shuffle::walk(q, trailing, low(live)).is_some()
    }
}

/// [`crate::skip`]'s nibble-set classifier, one 16-byte step at a time.
///
/// # Safety
///
/// Requires SSSE3, probed by [`crate::skip::wide::find`] before dispatch. Loads
/// are full 16-byte reads from `chunks_exact(16)`; both shuffle indices are masked
/// to `0..16`, so `pshufb`'s high-bit zeroing behavior is unreachable.
#[target_feature(enable = "ssse3")]
pub(crate) unsafe fn classify(lo: &[u8; 16], hi: &[u8; 16], hay: &[u8]) -> Option<usize> {
    // SAFETY: caller requires SSSE3, which the `# Safety` doc above makes a
    // precondition of calling this function at all. Every load reads exactly
    // 16 bytes from `lo`/`hi` (`[u8; 16]`) or a `chunks_exact(STEP)` window, so no
    // load runs past its source.
    unsafe {
        let load = |p: *const u8| _mm_loadu_si128(p.cast::<__m128i>());
        let (lo_tbl, hi_tbl) = (load(lo.as_ptr()), load(hi.as_ptr()));
        let nibble = _mm_set1_epi8(0x0F);
        for (i, block) in hay.chunks_exact(STEP).enumerate() {
            let v = load(block.as_ptr());
            let picked = _mm_shuffle_epi8(lo_tbl, _mm_and_si128(v, nibble));
            // `srli_epi16` shifts 16-bit lanes, so the mask is what makes this a
            // per-byte high nibble rather than a neighbor's bits leaking in.
            let high = _mm_and_si128(_mm_srli_epi16::<4>(v), nibble);
            let select = _mm_shuffle_epi8(hi_tbl, high);
            // No `vtst` here: equal-to-zero inverted is the same predicate.
            let zero = _mm_cmpeq_epi8(_mm_and_si128(picked, select), _mm_setzero_si128());
            let miss = _mm_movemask_epi8(zero) as u32;
            if miss != 0xFFFF {
                return Some(i * STEP + (!miss & 0xFFFF).trailing_zeros() as usize);
            }
        }
        crate::skip::wide::tail(lo, hi, hay)
    }
}
