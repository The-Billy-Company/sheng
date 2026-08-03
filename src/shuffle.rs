//! The register-resident kernel: one shuffle per byte, no gather, no branch.
//!
//! A quotient has at most 16 blocks, so one transition row is 16 bytes — exactly
//! one SIMD register, and `shuffle(row, v)` applies that row to all 16 lanes at
//! once. That is Langdale's Sheng execution technique (*Say Hello To My Little
//! Friend*, 2018), applied here to an over-approximation rather than to an exact
//! small DFA. The row is a 16-byte load from a 4 KiB table that stays L1-resident,
//! so no step ever waits on a dependent load out of a full-size transition table.
//!
//! # What the register holds, and why that is the whole speed story
//!
//! The obvious thing to keep in the register is the **state**, broadcast to every
//! lane. It works, and it is what this kernel used to do — but it is a dead end,
//! because each shuffle needs the previous shuffle's answer. One byte per shuffle
//! *latency*, forever. Measured on an M4 that is 2 cycles a byte, and no amount of
//! unrolling moves it: there is one dependency chain and it is as long as the
//! document.
//!
//! So the register holds the **function** instead. Seed it with the identity
//! `[0,1,…,15]` and the same single shuffle per byte now composes rather than
//! steps: after any run of bytes, lane `i` holds *the block that run would reach if
//! it had started in block `i`* — all sixteen answers, for the price of the one.
//! Nothing in the loop depends on where the scan actually is, so the haystack
//! splits into `WAYS` slices that advance in lockstep with no dependency between
//! them, and the shuffle unit sees `WAYS` independent chains instead of one. The
//! per-byte instruction count is unchanged; only the critical path shrinks.
//!
//! Composition is closed under the same instruction, which is what makes the
//! bookkeeping free: `shuffle(g, f)` is `g ∘ f`, so collapsing the slices back into
//! one real trajectory at the end of a chunk costs a handful of shuffles per
//! `CHUNK` bytes.
//!
//! This is Mytkowicz, Musuvathi & Schulte's enumerative technique (*Data-parallel
//! finite-state machines*, ASPLOS 2014) — transitions from all states at once, as a
//! gather, implemented with a byte shuffle where no gather exists. Their overhead is
//! proportional to the state count, which is why they need convergence optimizations
//! to recover it; a sieve has already capped its machine at `LANES` blocks to fit
//! the register at all, so here enumeration is free. The bound that makes the filter
//! sound is the same one that makes it parallel.
//!
//! # The accept test is still exact, and still free
//!
//! Blocks were renumbered by the lattice harvest so every accepting block sorts
//! above every non-accepting one, which makes "did this ever accept?" an unsigned
//! **max** — one instruction per byte, off the critical path.
//!
//! Under function composition that max is per lane, so `high[i]` is the highest
//! block the slice would have visited *starting from `i`*. Resolving the chunk
//! walks the real state through the slices in order, reading each slice's max at
//! exactly the lane the real trajectory entered it on. The state entering slice `w`
//! is real by construction — it is the previous slices' composition applied to a
//! real state — so what comes out is the true maximum over the chunk, not a bound
//! on it. This kernel therefore refutes exactly the documents the scalar reference
//! refutes; the differential test in `tests/soundness.rs` holds it to that.
//!
//! Reading the whole 16-lane max instead would also be *sound* — it can only
//! over-report an accept, which costs a skip and never a wrong answer — but it
//! would be reading sixteen hypothetical scans as if they had all happened, and on
//! an unanchored pattern nearly every chunk would fail to refute. Selectivity is
//! the product here, so the kernel pays the few shuffles to keep it.
//!
//! Deleting the max altogether is possible and **measured not to be worth it**. Give
//! every accepting block a self-loop and "did it ever accept" collapses into the
//! final state, so the per-byte work drops from load+shuffle+max to load+shuffle.
//! That variant benchmarks at 0.131 ns/byte against this one's 0.134 — inside the
//! noise, because the loop is bound by the row load and the shuffle port, not by the
//! spare integer op riding beside them. It would have cost a second, trapping form
//! of every quotient (the selectivity chain needs the honest one, since an
//! absorbing accept drives its long-run rate to 1) for two percent. Left undone
//! deliberately.

use crate::arch;
use crate::lattice::{LANES, Quotient};
use crate::skip::Skip;

/// Which byte-shuffle instruction set [`refutes`] dispatches to on the machine
/// that is running. See [`crate::arch::Kernel`] for what each variant means and
/// why it is reported rather than assumed.
pub use crate::arch::Kernel;

/// What [`refutes`] will actually run here. See [`crate::arch::kernel`].
pub use crate::arch::kernel;

/// Bytes between accept checks. One resolution per chunk instead of per byte; the
/// per-lane max cannot lose an accept inside a chunk, so the only cost of a larger
/// chunk is overshooting past the first accepting position — and a refutation
/// filter does not care where the match was, only whether one is possible.
pub(crate) const CHUNK: usize = 256;

/// Independent composition chains advanced in lockstep.
///
/// This is the ILP dial, and it is set by the shuffle unit rather than by taste: a
/// chain issues one shuffle every `latency` cycles, so it takes about `latency`
/// chains to saturate a unit that retires one shuffle per cycle. Four covers the
/// 2-cycle `tbl` on Apple silicon and the 1-cycle `pshufb` on x86_64 with margin,
/// while keeping the working set — `WAYS` function registers plus `WAYS` max
/// registers plus the rows in flight — well inside sixteen architectural vector
/// registers, so nothing spills on the narrower of the two targets.
///
/// Swept on an M4 over 32 MiB of real source (`cargo run --release --example bench`,
/// geomean ns/byte): two 0.160, **four 0.134**, six 0.133, eight 0.140. Two is short
/// of covering the latency and eight starts costing more in register pressure and
/// short-document setup than the extra chains return, so the curve is flat exactly
/// where the latency argument says it should be.
pub(crate) const WAYS: usize = 4;

/// Bytes each chain walks per full chunk. The slices tile the chunk exactly, so the
/// four streams are four sequential reads and the prefetcher sees what it expects.
///
/// A short final chunk re-derives its own stride rather than falling back to the
/// scalar walk — a 64-byte document is entirely "final chunk", and handing that case
/// to the reference path made small documents several times *slower* than the kernel
/// they were supposed to be using.
pub(crate) const STRIDE: usize = CHUNK / WAYS;

/// The do-nothing function: lane `i` holds `i`, so shuffling by it is identity and
/// shuffling it by a row yields that row. Every chain starts here.
pub(crate) const IDENTITY: [u8; LANES] = {
    let mut v = [0u8; LANES];
    let mut i = 0;
    while i < LANES {
        {
            v[i] = i as u8;
        }
        i += 1;
    }
    v
};

/// Does `q` **prove** `hay` holds no match?
///
/// `true` is a conclusive negative: the quotient recognizes a superset of the
/// pattern's language, so if the quotient never accepts, nothing does. `false`
/// means "cannot rule it out" and obliges the caller to run a real matcher — it
/// is never evidence of a match.
#[must_use]
pub fn refutes(q: &Quotient, hay: &[u8]) -> bool {
    // Dispatch reads [`kernel`] rather than re-deriving the cfg ladder, so what runs
    // and what the crate reports are the same decision. NEON needs no runtime probe
    // (it is baseline on aarch64); SSSE3's probe caches in a static after the first
    // call, and either way this is once per document, not once per byte.
    match kernel() {
        #[cfg(target_arch = "aarch64")]
        // SAFETY: this arm is only reachable under `#[cfg(target_arch = "aarch64")]`,
        // where NEON is baseline — exactly `arch::neon::sweep_shuffle`'s own
        // precondition.
        Kernel::Neon => unsafe { arch::neon::sweep_shuffle(q, hay) },
        #[cfg(target_arch = "x86_64")]
        // SAFETY: `kernel()` returns `Ssse3` only after `is_x86_feature_detected!`
        // confirmed the CPU has it — exactly `arch::ssse3::sweep_shuffle`'s own
        // precondition.
        Kernel::Ssse3 => unsafe { arch::ssse3::sweep_shuffle(q, hay) },
        _ => scalar(q, hay),
    }
}

/// [`refutes`], but skipping the runs the quotient provably sits still through.
///
/// The loop is the classical accelerated-DFA shape: while the run is in the block
/// `skip` describes, ask [`Skip::find`] where the next byte that moves it is and
/// jump there; otherwise step. It is **exact, not approximate** — a self-loop byte
/// in a non-accepting block cannot change the state and cannot visit an accepting
/// block, so the bytes jumped over could not have contributed an answer.
///
/// Two preconditions, both established by [`Skip::of`] and the caller that stores
/// one: `skip.resident` must be non-accepting, and its escape set must be exactly
/// the bytes that leave it. A skip that overshoots would make the sieve reject a
/// document that matches, so neither is left to chance —
/// `tests/soundness.rs` runs this against [`scalar`] on every pattern that
/// harvests, and [`crate::Skip`] holds the instrument to its own definition.
///
/// Whether this beats [`refutes`] is an economic question, not a universal one:
/// the excursions between skips are walked one byte at a time, so a quotient that
/// leaves its resident block often pays more here than the four-way composition
/// costs. [`crate::price`] decides, and `examples/bench.rs` measures.
#[must_use]
pub fn refutes_skipping(q: &Quotient, skip: &Skip, hay: &[u8]) -> bool {
    let mut at = 0;
    let mut state = q.start;
    while at < hay.len() {
        if state == skip.resident {
            // `None` retires every remaining byte without reading it: nothing in
            // the rest of the document can move the run out of a block that
            // does not accept.
            let Some(step) = skip.find(&hay[at..]) else {
                return true;
            };
            at += step;
        }
        state = q.rows[usize::from(hay[at])][usize::from(state)];
        if state >= q.threshold {
            return false;
        }
        at += 1;
    }
    true
}

/// The reference semantics every vector path is checked against. Kept in the
/// shipping build, not behind `cfg(test)`: it is the fallback on any target
/// without a byte shuffle, and a differential test can only be honest if it
/// exercises the same code the fallback runs.
#[must_use]
pub fn scalar(q: &Quotient, hay: &[u8]) -> bool {
    let mut state = q.start;
    for chunk in hay.chunks(CHUNK) {
        match walk(q, chunk, state) {
            Some(reached) => state = reached,
            None => return false,
        }
    }
    true
}

/// Step `q` across `hay` from `state`, or `None` the moment the run is known to
/// have visited an accepting block.
///
/// One chunk's worth of the scalar semantics, factored out because the vector
/// kernels in [`crate::arch`] need exactly it for the sub-chunk tail — a remainder
/// too short to slice is not worth a second code path to get wrong.
pub(crate) fn walk(q: &Quotient, hay: &[u8], mut state: u8) -> Option<u8> {
    let mut high = state;
    for &byte in hay {
        state = q.rows[usize::from(byte)][usize::from(state)];
        high = high.max(state);
    }
    (high < q.threshold).then_some(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identity_shuffles_to_itself() {
        for (i, &lane) in IDENTITY.iter().enumerate() {
            assert_eq!(usize::from(lane), i);
        }
    }

    /// The slices have to tile a chunk exactly — a leftover byte would be silently
    /// unscanned, which is the one bug in here that could turn a refutation wrong.
    #[test]
    fn the_slices_tile_a_chunk_with_nothing_left_over() {
        assert_eq!(STRIDE * WAYS, CHUNK);
    }
}
