//! AVX-512 kernels: probed at runtime by [`super::kernel`] — and, like
//! [`super::avx2`] and unlike [`super::ssse3`], probed against the *operating system*
//! as well as the silicon — before either of these is ever called.
//!
//! # Four slices per register, for the same reason AVX2 gets two
//!
//! `vpshufb` is defined **per 128-bit lane** at every width it exists at, so a `zmm`
//! holds four independent byte shuffles rather than one 64-byte one. That is again
//! exactly the wanted semantics rather than the usual annoyance: [`crate::shuffle`]'s
//! register holds a *function over sixteen blocks*, and four independent functions is
//! precisely what four more slices need. One `zmm` therefore carries four composition
//! chains, and this kernel runs [`WAYS`] slices — four times [`shuffle::WAYS`] — out
//! of the same four registers SSSE3 uses for one apiece.
//!
//! `vpermb` (AVX512-VBMI) is deliberately **not** used, and the reason is the same one
//! that caps the lane count. A cross-lane permute buys a *wider table*, and the table
//! here is [`LANES`](crate::lattice::LANES) entries because that is the block count of
//! a quotient, not because 16 is what a register happened to hold. There is no bigger
//! machine to address — only more slices of the same one, which `vpshufb` already
//! gives. Declining VBMI also keeps the probe on `avx512f` + `avx512bw`, which is a
//! strictly wider set of silicon than VBMI's, at no cost in per-byte work.
//!
//! # What the width buys, and what it does not
//!
//! Per byte, the shuffle and the `max` now cover four bytes instead of one, against
//! three extra `vinserti32x4` to stack the four rows — so the arithmetic per byte
//! falls while the two loads per byte, which is what the loop is actually bound by, do
//! not move at all. That is a smaller win than the width suggests, and on some silicon
//! it is a loss: a `zmm` shuffle can hold the core at a lower frequency for the rest of
//! the document, which no per-byte instruction count can see. Whether it is worth
//! dispatching to is therefore [`crate::price`]'s question and nobody else's, and the
//! answer is allowed to be no — `super::kernel` will not select this kernel until
//! `price::MINTED` holds a row measured on it, and if that row prices it above
//! [`super::avx2`] the ladder simply never reaches it.

use core::arch::x86_64::{
    __m128i, __m512i, __mmask64, _mm_loadu_si128, _mm_max_epu8, _mm_set1_epi8, _mm_shuffle_epi8,
    _mm512_and_si512, _mm512_broadcast_i32x4, _mm512_castsi128_si512, _mm512_extracti32x4_epi32,
    _mm512_inserti32x4, _mm512_loadu_si512, _mm512_max_epu8, _mm512_set1_epi8, _mm512_shuffle_epi8,
    _mm512_srli_epi16, _mm512_test_epi8_mask,
};

use super::ssse3::low;
use crate::lattice::Quotient;
use crate::shuffle::{self, CHUNK, IDENTITY};

/// Bytes per vector step: the four 128-bit lanes of one `zmm`, which is what
/// `_mm512_loadu_si512` reads and `vpshufb` classifies at once.
const STEP: usize = super::STEP * 4;

/// Composition chains advanced in lockstep — four times [`shuffle::WAYS`], because
/// each of those registers now holds four independent 16-lane functions rather than
/// one. The register pressure, which is what set the four in the first place, is
/// unchanged.
const WAYS: usize = shuffle::WAYS * 4;

/// Bytes each chain walks per full chunk. A quarter of [`shuffle::STRIDE`], since four
/// times as many slices tile the same [`CHUNK`].
const STRIDE: usize = CHUNK / WAYS;

/// [`crate::shuffle::refutes`]'s composition kernel, four slices to a register. The
/// scalar meaning is [`shuffle::scalar`]'s and the differential tests hold it there.
///
/// # Safety
///
/// Requires AVX-512 F and BW, which [`super::kernel`] probes for — the two silicon
/// bits, `OSXSAVE`, and the three `XCR0` bits that are the operating system's promise
/// to preserve the opmask and the upper `zmm` state — before dispatching here. The only
/// genuinely unchecked operations are the row loads, and a row is exactly
/// [`LANES`](crate::lattice::LANES) bytes indexed by a `u8`.
#[target_feature(enable = "avx512f,avx512bw")]
pub(crate) unsafe fn sweep_shuffle(q: &Quotient, hay: &[u8]) -> bool {
    /// The 64-byte register holding one transition row per 128-bit lane, in slice
    /// order — the one place this kernel pays for its width.
    ///
    /// Four explicit parameters rather than a `[*const u8; 4]` built by
    /// [`core::array::from_fn`], which is [`super::avx2`]'s `pair` shape and is load-
    /// bearing rather than cosmetic. `from_fn` here did not inline: a release build put
    /// eight `callq`s to its `FnMut` machinery in this kernel's hot loop, and the
    /// unindexable closure kept LLVM from unrolling the four-chain loop, which in turn
    /// made `compose` and `high` addressable and spent a 64-byte spill and reload on each
    /// of them every step —
    ///
    ///     vpshufb   (%rax,%rbx), %zmm0, %zmm0   ; compose[reg], from the stack
    ///     vmovdqa64 %zmm0, (%rax,%rbx)          ; and straight back to it
    ///
    /// — on a target with thirty-two `zmm` registers and eight of them wanted. That is
    /// why this kernel merely matched `avx2` (0.335 against 0.290 ns/B) despite reading
    /// four times the bytes per step: not the width, and not the frequency, but a hot
    /// loop that went through memory. `avx2` and `ssse3` never showed it because neither
    /// needed more than two rows, so neither reached for `from_fn`.
    ///
    /// # Safety
    ///
    /// Every pointer must be readable for [`LANES`](crate::lattice::LANES) bytes.
    #[inline(always)]
    unsafe fn quad(a: *const u8, b: *const u8, c: *const u8, d: *const u8) -> __m512i {
        // SAFETY: the caller guarantees each pointer addresses a full 16-byte row.
        unsafe {
            let load = |p: *const u8| _mm_loadu_si128(p.cast::<__m128i>());
            let z = _mm512_castsi128_si512(load(a));
            let z = _mm512_inserti32x4::<1>(z, load(b));
            let z = _mm512_inserti32x4::<2>(z, load(c));
            _mm512_inserti32x4::<3>(z, load(d))
        }
    }

    /// The `aarch64` and SSSE3 `sweep`, with each register carrying four slices. See
    /// [`super::neon::sweep_shuffle`] for what is being composed and why.
    #[inline(always)]
    unsafe fn sweep(q: &Quotient, block: &[u8], stride: usize, mut live: __m128i) -> (__m128i, u8) {
        // SAFETY: caller requires AVX-512 (this fn is only called from
        // `sweep_shuffle`, itself gated on `#[target_feature(enable =
        // "avx512f,avx512bw")]`). Every load reads exactly `LANES` bytes from
        // `IDENTITY` or a `q.rows` row, both `[u8; LANES]` arrays, so no load runs
        // past its 16-byte source.
        unsafe {
            let identity =
                _mm512_broadcast_i32x4(_mm_loadu_si128(IDENTITY.as_ptr().cast::<__m128i>()));
            let mut compose = [identity; shuffle::WAYS];
            let mut high = [identity; shuffle::WAYS];
            for step in 0..stride {
                // Indexed over a constant trip count rather than
                // `compose.iter_mut().zip(&mut high)`, for the same reason `quad` takes
                // four parameters: the zip did not inline either. A release build left two
                // `Zip::new` calls in this kernel, and an iterator LLVM cannot see through
                // is an iterator it will not unroll, which is what made `compose` and
                // `high` addressable. Four constant-index updates keep both in `zmm`.
                for reg in 0..shuffle::WAYS {
                    // This register carries slices `4*reg ..= 4*reg+3`, whose bytes
                    // are therefore one stride apart from each other.
                    let at = 4 * reg * stride + step;
                    let row = |lane: usize| q.rows[usize::from(block[at + lane * stride])].as_ptr();
                    let rows = quad(row(0), row(1), row(2), row(3));
                    compose[reg] = _mm512_shuffle_epi8(rows, compose[reg]);
                    high[reg] = _mm512_max_epu8(high[reg], compose[reg]);
                }
            }
            // Collapse in slice order — lane 0 through lane 3, register by register —
            // so each slice's max is read at the lane the real trajectory actually
            // entered it on. Any other order would still be sound and would stop being
            // exact, which is the distinction `crate::shuffle` explains.
            let mut seen = live;
            for (f, h) in compose.into_iter().zip(high) {
                let lanes = [
                    (
                        _mm512_extracti32x4_epi32::<0>(f),
                        _mm512_extracti32x4_epi32::<0>(h),
                    ),
                    (
                        _mm512_extracti32x4_epi32::<1>(f),
                        _mm512_extracti32x4_epi32::<1>(h),
                    ),
                    (
                        _mm512_extracti32x4_epi32::<2>(f),
                        _mm512_extracti32x4_epi32::<2>(h),
                    ),
                    (
                        _mm512_extracti32x4_epi32::<3>(f),
                        _mm512_extracti32x4_epi32::<3>(h),
                    ),
                ];
                for (slice, visited) in lanes {
                    seen = _mm_max_epu8(seen, _mm_shuffle_epi8(visited, live));
                    live = _mm_shuffle_epi8(slice, live);
                }
            }
            (live, low(seen))
        }
    }

    // Block ids are all below LANES, so no shuffle index ever has its high bit set
    // and `vpshufb`'s zeroing behavior is unreachable.
    // SAFETY: caller requires AVX-512, which `#[target_feature(enable =
    // "avx512f,avx512bw")]` above makes a precondition of calling this function at
    // all. `sweep` carries its own proof; `low` and `walk` are safe.
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

/// [`crate::skip`]'s nibble-set classifier, one 64-byte step at a time.
///
/// The two tables are broadcast to all four lanes of the register, because `vpshufb`
/// indexes within its own 128-bit lane — the same property that lets the kernel above
/// carry four slices makes this a plain quadrupling of the step.
///
/// The membership test is one instruction here rather than three. `vptestmb` writes
/// "these lanes ANDed nonzero" straight into a mask register, which is exactly
/// [`crate::skip::wide::member`]; the narrower kernels have no mask register to write
/// it to and must compare against zero and invert instead.
///
/// # Safety
///
/// Requires AVX-512 F and BW, probed by [`crate::skip::wide::find`] before dispatch.
/// Loads are full 64-byte reads from `chunks_exact(STEP)`; both shuffle indices are
/// masked to `0..16`, so `vpshufb`'s high-bit zeroing behavior is unreachable.
#[target_feature(enable = "avx512f,avx512bw")]
pub(crate) unsafe fn classify(lo: &[u8; 16], hi: &[u8; 16], hay: &[u8]) -> Option<usize> {
    // SAFETY: caller requires AVX-512, which the `# Safety` doc above makes a
    // precondition of calling this function at all. Every load reads exactly
    // 16 bytes from `lo`/`hi` (`[u8; 16]`) or 64 from a `chunks_exact(STEP)` window,
    // so no load runs past its source.
    unsafe {
        let spread = |p: *const u8| _mm512_broadcast_i32x4(_mm_loadu_si128(p.cast::<__m128i>()));
        let (lo_tbl, hi_tbl) = (spread(lo.as_ptr()), spread(hi.as_ptr()));
        let nibble = _mm512_set1_epi8(0x0F);
        for (i, block) in hay.chunks_exact(STEP).enumerate() {
            let v = _mm512_loadu_si512(block.as_ptr().cast::<__m512i>());
            let picked = _mm512_shuffle_epi8(lo_tbl, _mm512_and_si512(v, nibble));
            // `srli_epi16` shifts 16-bit lanes, so the mask is what makes this a
            // per-byte high nibble rather than a neighbor's bits leaking in.
            let high = _mm512_and_si512(_mm512_srli_epi16::<4>(v), nibble);
            let select = _mm512_shuffle_epi8(hi_tbl, high);
            let hit: __mmask64 = _mm512_test_epi8_mask(picked, select);
            if hit != 0 {
                return Some(i * STEP + hit.trailing_zeros() as usize);
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
    /// four times as many slices as [`shuffle::STRIDE`] describes.
    #[test]
    fn four_times_as_many_slices_still_tile_a_chunk_exactly() {
        assert_eq!(STRIDE * WAYS, CHUNK);
        assert_eq!(WAYS % 4, 0, "each register carries exactly four slices");
    }
}
