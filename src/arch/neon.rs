//! NEON kernels: baseline on every `aarch64` target, so neither of these needs a
//! runtime probe — the caller's `cfg(target_arch = "aarch64")` is the whole
//! precondition.

use core::arch::aarch64::{
    uint8x16_t, vandq_u8, vdupq_n_u8, vget_lane_u64, vgetq_lane_u8, vld1q_u8, vmaxq_u8, vqtbl1q_u8,
    vreinterpret_u64_u8, vreinterpretq_u16_u8, vshrn_n_u16, vshrq_n_u8, vtstq_u8,
};

use super::STEP;
use crate::lattice::Quotient;
use crate::shuffle::{self, CHUNK, IDENTITY, STRIDE, WAYS};

/// [`crate::shuffle::refutes`]'s composition kernel. See that module's
/// documentation for what the register holds and why.
///
/// # Safety
///
/// Requires NEON, which is baseline on every `aarch64` target, so the caller's
/// `cfg` is the whole precondition. The only genuinely unchecked operation is the
/// row load, and a row is exactly [`LANES`](crate::lattice::LANES) bytes indexed
/// by a `u8`.
#[target_feature(enable = "neon")]
pub(crate) unsafe fn sweep_shuffle(q: &Quotient, hay: &[u8]) -> bool {
    /// Compose `WAYS` slices of `stride` bytes each, then collapse them onto the one
    /// trajectory that happened. Returns the state it leaves the block in and the
    /// highest block that trajectory actually visited.
    ///
    /// Inlined at both call sites so the full-chunk one folds `stride` to a constant
    /// and unrolls, while the short final chunk keeps the same code with a stride it
    /// only learns at runtime.
    #[inline(always)]
    unsafe fn sweep(
        q: &Quotient,
        block: &[u8],
        stride: usize,
        mut live: uint8x16_t,
    ) -> (uint8x16_t, u8) {
        // SAFETY: caller requires NEON (this fn is only called from `sweep_shuffle`,
        // itself gated on `#[target_feature(enable = "neon")]`). Every load reads
        // exactly `LANES` bytes from `IDENTITY` or a `q.rows` row, both
        // `[u8; LANES]` arrays, so no load runs past its 16-byte source.
        unsafe {
            let identity = vld1q_u8(IDENTITY.as_ptr());
            let mut compose = [identity; WAYS];
            let mut high = [identity; WAYS];
            for step in 0..stride {
                // Fixed-width arrays: this unrolls into WAYS independent
                // load/shuffle/max triples with no dependency between them.
                for (way, (f, h)) in compose.iter_mut().zip(&mut high).enumerate() {
                    let byte = block[way * stride + step];
                    let row = vld1q_u8(q.rows[usize::from(byte)].as_ptr());
                    *f = vqtbl1q_u8(row, *f);
                    *h = vmaxq_u8(*h, *f);
                }
            }
            // Reading `high[w]` at `live` — before advancing it — is what keeps this
            // exact rather than merely sound.
            let mut seen = live;
            for (f, h) in compose.into_iter().zip(high) {
                seen = vmaxq_u8(seen, vqtbl1q_u8(h, live));
                live = vqtbl1q_u8(f, live);
            }
            // Every lane of `seen` is a broadcast maxed with broadcasts, so any lane
            // is the answer and no horizontal reduction is needed.
            (live, vgetq_lane_u8::<0>(seen))
        }
    }

    // SAFETY: caller requires NEON, which `#[target_feature(enable = "neon")]`
    // above makes a precondition of calling this function at all. `sweep` carries
    // its own proof; `walk` and `vgetq_lane_u8::<0>` are safe.
    unsafe {
        // The real trajectory, broadcast — the one value in here that is a state
        // rather than a function.
        let mut live = vdupq_n_u8(q.start);
        let mut rest = hay;

        while rest.len() >= CHUNK {
            let (chunk, after) = rest.split_at(CHUNK);
            let (next, seen) = sweep(q, chunk, STRIDE, live);
            if seen >= q.threshold {
                return false;
            }
            (live, rest) = (next, after);
        }

        // Fewer than WAYS bytes cannot be sliced at all; the scalar finish covers
        // that and the up-to-WAYS-1 bytes the even slicing leaves over.
        let (paved, trailing) = rest.split_at(rest.len() / WAYS * WAYS);
        let (live, seen) = sweep(q, paved, paved.len() / WAYS, live);
        if seen >= q.threshold {
            return false;
        }
        shuffle::walk(q, trailing, vgetq_lane_u8::<0>(live)).is_some()
    }
}

/// [`crate::skip`]'s nibble-set classifier, one 16-byte step at a time.
///
/// # Safety
///
/// Requires NEON (baseline on `aarch64`). Every load is a full 16-byte read from a
/// `chunks_exact(STEP)` window, and both shuffle indices are masked into `0..16`,
/// so no lane can address outside its table.
#[target_feature(enable = "neon")]
pub(crate) unsafe fn classify(lo: &[u8; 16], hi: &[u8; 16], hay: &[u8]) -> Option<usize> {
    // SAFETY: caller requires NEON, which the `# Safety` doc above makes a
    // precondition of calling this function at all. Every load reads exactly
    // 16 bytes from `lo`/`hi` (`[u8; 16]`) or a `chunks_exact(STEP)` window, so no
    // load runs past its source.
    unsafe {
        let (lo_tbl, hi_tbl) = (vld1q_u8(lo.as_ptr()), vld1q_u8(hi.as_ptr()));
        let nibble = vdupq_n_u8(0x0F);
        for (i, block) in hay.chunks_exact(STEP).enumerate() {
            let v = vld1q_u8(block.as_ptr());
            let picked = vqtbl1q_u8(lo_tbl, vandq_u8(v, nibble));
            let select = vqtbl1q_u8(hi_tbl, vshrq_n_u8::<4>(v));
            // `vtst` is the `& != 0` of `member`, done sixteen lanes at a time.
            let hit = vtstq_u8(picked, select);
            // NEON has no movemask. Narrowing 16 lanes to 4 bits each packs the
            // answer into one 64-bit word whose trailing zeros locate the lane.
            let packed = vget_lane_u64::<0>(vreinterpret_u64_u8(vshrn_n_u16::<4>(
                vreinterpretq_u16_u8(hit),
            )));
            if packed != 0 {
                return Some(i * STEP + (packed.trailing_zeros() as usize >> 2));
            }
        }
        crate::skip::wide::tail(lo, hi, hay, STEP)
    }
}
