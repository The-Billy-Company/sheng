//! AVX2 kernels: probed at runtime by [`super::kernel`] — and, unlike [`super::ssse3`],
//! probed against the *operating system* as well as the silicon — before either of
//! these is ever called.
//!
//! # Two slices per register, not two chunks per pass
//!
//! `vpshufb` shuffles each 128-bit half of its register **independently**: there is no
//! carry, no cross-lane index, and a lane's byte can only name a table entry from its
//! own half. For most algorithms that is the notorious annoyance of AVX2; for this one
//! it is exactly the wanted semantics, because [`crate::shuffle`]'s register holds a
//! *function over sixteen blocks* and two independent functions is precisely what a
//! second slice needs. So one `ymm` carries two composition chains, and the kernel
//! runs [`WAYS`] slices — twice [`shuffle::WAYS`] — out of the same four registers
//! SSSE3 uses for half as many.
//!
//! What that buys is per-byte work, not parallelism: both kernels keep four
//! independent register chains, so the ILP argument in [`crate::shuffle`] is unchanged.
//! The shuffle and the max now cover two bytes instead of one, against one extra
//! `vinserti128` to pair the two rows — so the arithmetic per byte falls by roughly a
//! sixth while the two loads per byte, which is what the loop is actually bound by,
//! do not move at all. Whether that is worth dispatching to is [`crate::price`]'s
//! question and nobody else's: `super::kernel` will not select this kernel until
//! `price::MINTED` holds a row that was measured on it.
//!
//! The 16-lane cap itself is **not** relaxed here and cannot be. It is the block
//! count of a quotient — `lattice::LANES` — not a register width, so a wider register
//! buys more slices of the same machine, never a bigger machine.

use core::arch::x86_64::{
    __m128i, __m256i, _mm_loadu_si128, _mm_max_epu8, _mm_set1_epi8, _mm_shuffle_epi8,
    _mm256_and_si256, _mm256_broadcastsi128_si256, _mm256_castsi128_si256, _mm256_castsi256_si128,
    _mm256_cmpeq_epi8, _mm256_extracti128_si256, _mm256_inserti128_si256, _mm256_loadu_si256,
    _mm256_max_epu8, _mm256_movemask_epi8, _mm256_set1_epi8, _mm256_setzero_si256,
    _mm256_shuffle_epi8, _mm256_srli_epi16,
};

use super::ssse3::low;
use crate::lattice::Quotient;
use crate::shuffle::{self, CHUNK, IDENTITY};

/// Bytes per vector step: two 128-bit halves of one `ymm`, which is what
/// `_mm256_loadu_si256` reads and `vpshufb` classifies at once.
const STEP: usize = super::STEP * 2;

/// Composition chains advanced in lockstep — twice [`shuffle::WAYS`], because each of
/// those registers now holds two independent 16-lane functions rather than one. The
/// register pressure, which is what set the four in the first place, is unchanged.
const WAYS: usize = shuffle::WAYS * 2;

/// Bytes each chain walks per full chunk. Half [`shuffle::STRIDE`], since twice as
/// many slices tile the same [`CHUNK`].
const STRIDE: usize = CHUNK / WAYS;

/// [`crate::shuffle::refutes`]'s composition kernel, two slices to a register. The
/// scalar meaning is [`shuffle::scalar`]'s and the differential tests hold it there.
///
/// # Safety
///
/// Requires AVX2, which [`super::kernel`] probes for — silicon bit, `OSXSAVE`, and the
/// `XCR0` upper-half promise — before dispatching here. The only genuinely unchecked
/// operations are the row loads, and a row is exactly
/// [`LANES`](crate::lattice::LANES) bytes indexed by a `u8`.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn sweep_shuffle(q: &Quotient, hay: &[u8]) -> bool {
    /// The 32-byte register holding slice `2w`'s transition row in its low half and
    /// slice `2w+1`'s in its high half — the one place this kernel pays for its width.
    ///
    /// # Safety
    ///
    /// Both pointers must be readable for [`LANES`](crate::lattice::LANES) bytes.
    #[inline(always)]
    unsafe fn pair(lo: *const u8, hi: *const u8) -> __m256i {
        // SAFETY: the caller guarantees both pointers address a full 16-byte row.
        unsafe {
            _mm256_inserti128_si256::<1>(
                _mm256_castsi128_si256(_mm_loadu_si128(lo.cast::<__m128i>())),
                _mm_loadu_si128(hi.cast::<__m128i>()),
            )
        }
    }

    /// The `aarch64` and SSSE3 `sweep`, with each register carrying two slices. See
    /// [`super::neon::sweep_shuffle`] for what is being composed and why.
    #[inline(always)]
    unsafe fn sweep(q: &Quotient, block: &[u8], stride: usize, mut live: __m128i) -> (__m128i, u8) {
        // SAFETY: caller requires AVX2 (this fn is only called from `sweep_shuffle`,
        // itself gated on `#[target_feature(enable = "avx2")]`). Every load reads
        // exactly `LANES` bytes from `IDENTITY` or a `q.rows` row, both `[u8; LANES]`
        // arrays, so no load runs past its 16-byte source.
        unsafe {
            let identity =
                _mm256_broadcastsi128_si256(_mm_loadu_si128(IDENTITY.as_ptr().cast::<__m128i>()));
            let mut compose = [identity; shuffle::WAYS];
            let mut high = [identity; shuffle::WAYS];
            for step in 0..stride {
                for (reg, (f, h)) in compose.iter_mut().zip(&mut high).enumerate() {
                    // This register carries slices `2*reg` and `2*reg+1`, whose bytes
                    // are therefore one stride apart.
                    let at = 2 * reg * stride + step;
                    let rows = pair(
                        q.rows[usize::from(block[at])].as_ptr(),
                        q.rows[usize::from(block[at + stride])].as_ptr(),
                    );
                    *f = _mm256_shuffle_epi8(rows, *f);
                    *h = _mm256_max_epu8(*h, *f);
                }
            }
            // Collapse in slice order — low half before high half, register by
            // register — so each slice's max is read at the lane the real trajectory
            // actually entered it on. Any other order would still be sound and would
            // stop being exact, which is the distinction `crate::shuffle` explains.
            let mut seen = live;
            for (f, h) in compose.into_iter().zip(high) {
                let halves = [
                    (_mm256_castsi256_si128(f), _mm256_castsi256_si128(h)),
                    (
                        _mm256_extracti128_si256::<1>(f),
                        _mm256_extracti128_si256::<1>(h),
                    ),
                ];
                for (slice, visited) in halves {
                    seen = _mm_max_epu8(seen, _mm_shuffle_epi8(visited, live));
                    live = _mm_shuffle_epi8(slice, live);
                }
            }
            (live, low(seen))
        }
    }

    // Block ids are all below LANES, so no shuffle index ever has its high bit set
    // and `vpshufb`'s zeroing behavior is unreachable.
    // SAFETY: caller requires AVX2, which `#[target_feature(enable = "avx2")]` above
    // makes a precondition of calling this function at all. `sweep` carries its own
    // proof; `low` and `walk` are safe.
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

/// [`crate::skip`]'s nibble-set classifier, one 32-byte step at a time.
///
/// The two tables are broadcast to both halves of the register, because `vpshufb`
/// indexes within its own half — the same property that lets the kernel above carry
/// two slices makes this a plain doubling of the step.
///
/// # Safety
///
/// Requires AVX2, probed by [`crate::skip::wide::find`] before dispatch. Loads are
/// full 32-byte reads from `chunks_exact(STEP)`; both shuffle indices are masked to
/// `0..16`, so `vpshufb`'s high-bit zeroing behavior is unreachable.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn classify(lo: &[u8; 16], hi: &[u8; 16], hay: &[u8]) -> Option<usize> {
    // SAFETY: caller requires AVX2, which the `# Safety` doc above makes a
    // precondition of calling this function at all. Every load reads exactly
    // 16 bytes from `lo`/`hi` (`[u8; 16]`) or 32 from a `chunks_exact(STEP)` window,
    // so no load runs past its source.
    unsafe {
        let spread =
            |p: *const u8| _mm256_broadcastsi128_si256(_mm_loadu_si128(p.cast::<__m128i>()));
        let (lo_tbl, hi_tbl) = (spread(lo.as_ptr()), spread(hi.as_ptr()));
        let nibble = _mm256_set1_epi8(0x0F);
        for (i, block) in hay.chunks_exact(STEP).enumerate() {
            let v = _mm256_loadu_si256(block.as_ptr().cast::<__m256i>());
            let picked = _mm256_shuffle_epi8(lo_tbl, _mm256_and_si256(v, nibble));
            // `srli_epi16` shifts 16-bit lanes, so the mask is what makes this a
            // per-byte high nibble rather than a neighbor's bits leaking in.
            let high = _mm256_and_si256(_mm256_srli_epi16::<4>(v), nibble);
            let select = _mm256_shuffle_epi8(hi_tbl, high);
            // No `vtst` here: equal-to-zero inverted is the same predicate.
            let zero = _mm256_cmpeq_epi8(_mm256_and_si256(picked, select), _mm256_setzero_si256());
            // A full 32 lanes to a mask, so unlike the SSSE3 path there are no
            // undefined high bits to mask off before inverting.
            let miss = _mm256_movemask_epi8(zero) as u32;
            if miss != u32::MAX {
                return Some(i * STEP + (!miss).trailing_zeros() as usize);
            }
        }
        crate::skip::wide::tail(lo, hi, hay, STEP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slices have to tile a chunk exactly — a leftover byte would be silently
    /// unscanned, which is the one bug in here that could turn a refutation wrong.
    /// Asserted for this kernel's own width, since it tiles the same [`CHUNK`] with
    /// twice as many slices as [`shuffle::STRIDE`] describes.
    #[test]
    fn twice_as_many_slices_still_tile_a_chunk_exactly() {
        assert_eq!(STRIDE * WAYS, CHUNK);
        assert_eq!(WAYS % 2, 0, "each register carries exactly two slices");
    }
}
