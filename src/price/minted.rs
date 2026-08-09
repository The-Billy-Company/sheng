//! The measured evidence: one [`Calibration`] row per (architecture, kernel) pair
//! anybody has actually timed, plus the fail-safe row for everyone else.

use super::calibration::{Calibration, REGIMES};
use crate::lattice::MAX_CONJUNCTS;
use crate::shuffle::Kernel;

/// Minted by `cargo run --release --example mint` over a large slice of real
/// source, each kernel timed alone as the minimum of several full traversals.
///
/// These numbers state the whole economics of this crate:
///
/// * the sieve beats the engine's per-byte walk by several times — a real
///   advantage, and the reason any of this pays;
/// * but the engine's *skip* is still an order of magnitude faster than the
///   sieve. Nothing that inspects every byte can front a `memchr`. That is not
///   a defect in the kernel; it is the arithmetic that decides where it belongs.
///
/// `dfa_excursion` is solved from a slate of lead bytes spanning two orders of
/// magnitude of frequency rather than assumed. Read at class resolution the
/// inverted values disagree by about tenfold; read from a per-byte table they
/// collapse into a narrow band — so that spread was the approximation talking,
/// and closing it is what makes a single coefficient defensible here.
///
/// The one-conjunct slot is unmeasured because the lattice harvest fills to
/// [`MAX_CONJUNCTS`] whenever it yields anything at all, so no pattern on the mint's
/// slate reaches it. [`Calibration::sieve_per_byte`] extrapolates it conservatively
/// rather than treating the hole as free.
///
/// Every figure still carries double-digit-percent run-to-run variance on a loaded
/// machine. Because the gate is scale-invariant, that variance costs no decisions:
/// a run under load inflates the absolute figures together.
///
/// A re-mint is a fresh complete measurement, not a splice of old and new
/// afternoons: the gate reads *ratios* between these numbers, and a ratio built
/// from two different sessions is not a measurement of anything. The exception is
/// `skip_excursion`, which is dimensionless and already self-normalized inside a
/// single interleaved timing window — `mint`'s `paired` re-times both baselines
/// against the pattern they divide, round by round — so it may be carried forward
/// when the rest of the row is re-taken. The higher of consecutive paired mints is
/// the one recorded, because an overstated excursion can only decline a skip.
///
/// Two coefficients carry a [`Residency`](super::Residency) index because a
/// `memchr` and a dense-DFA re-entry are both cheaper once the bytes are already
/// resident; `dfa_walk` and `sieve` do not, because a dependent-load walk and an
/// issue-bound composition kernel have no headroom a hotter haystack could give
/// them. `skip_excursion` is indexed for symmetry and is not expected to move —
/// it re-enters sixteen blocks resident in either regime.
///
/// # The mint can be fooled, and was
///
/// A first residency mint read both columns identical, which looked like "residency
/// does not matter on this silicon". It was not a finding: `mint` was aimed at a
/// tree smaller than either requested working set, so both columns were the same
/// bytes timed twice. `examples/mint.rs` now refuses a corpus too small to leave
/// last-level cache rather than printing a row that says the memory system does
/// not exist — a row is a claim about a memory system, and a mint that never
/// reached memory has no business making one. The same trap makes a cache-resident
/// re-mint look like the shipped memory-resident row went stale.
pub const MACOS_AARCH64_NEON: Calibration = Calibration {
    arch: "aarch64",
    kernel: Kernel::Neon,
    host: "macos aarch64 · 16 logical cores · Neon kernel",
    minted: "2026-08-09",
    dfa_skip: [0.012390, 0.017507],
    dfa_walk: 1.313341,
    dfa_excursion: [8.057903, 9.751283],
    skip_excursion: [[7.611088, 9.647507], [7.788777, 6.963965]],
    sieve: [0.0, 0.196478],
};

/// Native x86_64 Linux, timed the same way as [`MACOS_AARCH64_NEON`].
///
/// With both rows on the composing kernel, they read as a comparison of silicon
/// rather than of two different kernels. Absolute walk cost is nearly identical —
/// a dependent-load chain either way — and relative skip/walk can even favor
/// SSSE3; the sieve itself is where the architectures still disagree by a
/// noticeable fraction. Inheriting one machine's numbers on the other would still
/// misprice which patterns arm — which is the whole reason this crate keeps a row
/// per (architecture, kernel) pair instead of one default.
pub const LINUX_X86_64_SSSE3: Calibration = Calibration {
    arch: "x86_64",
    kernel: Kernel::Ssse3,
    host: "linux x86_64 · 20 logical cores · Ssse3 kernel",
    minted: "2026-08-03",
    // Not yet timed in the cache-resident regime — this box is not here to run a mint
    // on. Zero reads as unmeasured, so an `x86_64` caller declaring `Residency::Cache`
    // gets `Uncalibrated` rather than these memory-resident numbers, which is the same
    // refusal the crate makes about a machine it has never seen. `.github/workflows/mint.yml`
    // is where the pair gets filled in.
    dfa_skip: [0.0, 0.012845],
    dfa_walk: 1.251617,
    dfa_excursion: [0.0, 11.554774],
    skip_excursion: [[0.0, 8.849832], [0.0, 7.255182]],
    sieve: [0.0, 0.218482],
};

/// The answer for a machine nobody has measured: **nothing is known**, so the sieve
/// price reads infinite and every pattern declines.
///
/// This is deliberately not a guess averaged from the rows above. The ratios are an
/// instruction-set property, and a target absent from [`MINTED`] is one whose
/// `memchr`, dependent-load walk, and byte shuffle stand in a relationship nobody has
/// timed — including, most sharply, a target with no byte shuffle at all, where the
/// sieve runs [`crate::shuffle::scalar`] and any vector-measured coefficient would be
/// pure optimism. Callers who would rather measure than decline can mint their own
/// and pass it in a [`crate::Policy`]; `cargo run --release --example mint` prints the
/// row.
pub const UNMEASURED: Calibration = Calibration {
    arch: "unmeasured",
    kernel: Kernel::Scalar,
    host: "no machine — nothing here was measured",
    minted: "never",
    dfa_skip: [0.0; REGIMES],
    dfa_walk: 0.0,
    dfa_excursion: [0.0; REGIMES],
    skip_excursion: [[0.0; REGIMES]; 2],
    sieve: [0.0; MAX_CONJUNCTS],
};

/// Every (architecture, kernel) pair anybody has actually measured. [`super::active`]
/// picks from here by matching the running target; adding silicon means adding a
/// row, not editing a default.
///
/// The key is `(architecture, kernel)`, not `(os, architecture, kernel)` — Windows is
/// not a third row waiting to be minted, it is a claim that these same two already
/// cover it. `.github/workflows/native.yml` runs both rows' underlying (architecture,
/// kernel) pairs natively across all six Windows/Linux/macOS × x86_64/arm64 legs on
/// every push: each leg proves dispatch chose the matching vector kernel and that the
/// gate above armed no row that then lost against real source text. An OS column
/// would only earn its keep the day one leg measures a loss the other two don't —
/// evidence, not a guess that Windows differs, is what would justify it.
///
/// # This slice is also the dispatch ladder's permission list
///
/// [`crate::shuffle::kernel`] will not select a kernel that has no row here, so what
/// is *absent* from this slice is as load-bearing as what is present. Two consequences
/// worth stating out loud:
///
/// * A new instruction set lands without a flag day. `Kernel::Avx2` is implemented,
///   differentially tested against the scalar reference on real AVX2 silicon by
///   `tests/kernels.rs`, and **not dispatched to**, because no row below was measured
///   on it. Adding one arms it; until then it moves no decision, and — the failure this
///   ordering exists to prevent — it cannot win a dispatch on a machine whose only
///   calibration describes `pshufb` and thereby strand a modern x86_64 install on
///   [`UNMEASURED`].
/// * A row is therefore per *kernel*, not per machine, which is why these names carry
///   both halves of the key. One `cargo run --release --example mint` prints a row for
///   every kernel the running silicon has, so a machine's rows can be pasted in
///   together or one at a time without either one implying the other.
///   `.github/workflows/mint.yml` is that run, on real hardware for every
///   (architecture, kernel) pair this crate dispatches to.
pub const MINTED: &[Calibration] = &[MACOS_AARCH64_NEON, LINUX_X86_64_SSSE3];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::Residency;
    use crate::prior;

    /// Every row must describe a machine that could exist, and no two rows may claim
    /// the same (architecture, kernel) pair — [`super::super::active`] resolves by
    /// first match, so a duplicate would silently shadow a measurement.
    #[test]
    fn the_minted_rows_are_distinct_and_self_describing() {
        for (i, cal) in MINTED.iter().enumerate() {
            assert!(
                Residency::ALL.iter().any(|&at| cal.is_measured(at)),
                "{} row {i} measured nothing in any regime",
                cal.arch
            );
            assert!(
                cal.kernel.is_vector(),
                "a scalar-kernel row would price the vector economics wrongly"
            );
            assert!(
                MINTED[..i]
                    .iter()
                    .all(|seen| (seen.arch, seen.kernel) != (cal.arch, cal.kernel)),
                "duplicate calibration for {} / {:?}",
                cal.arch,
                cal.kernel
            );
        }
    }

    /// The safety property that lets an unpriced kernel exist in the tree at all:
    /// dispatch never elects one. Stated here rather than in [`crate::arch`] because it
    /// is a claim about *this slice* — the moment a row lands for a faster kernel the
    /// answer is allowed to change, and the moment one is deleted it must change back.
    #[test]
    fn dispatch_never_elects_a_kernel_this_slice_has_not_priced() {
        let chosen = crate::shuffle::kernel();
        let ladder = crate::shuffle::available();
        assert!(
            ladder.contains(&chosen),
            "dispatch chose {chosen:?}, which this silicon cannot execute"
        );
        let priced = |kernel| {
            MINTED
                .iter()
                .any(|c| c.arch == crate::price::ARCH && c.kernel == kernel)
        };
        if ladder.iter().copied().any(priced) {
            assert!(
                priced(chosen),
                "{} has a priced kernel but dispatch chose the unpriced {chosen:?}",
                crate::price::ARCH
            );
            // And the *fastest* such, or the ladder's order is decoration.
            let best = ladder.iter().copied().find(|&k| priced(k));
            assert_eq!(
                Some(chosen),
                best,
                "dispatch settled for {chosen:?} while {best:?} is both faster and priced"
            );
        }
    }

    /// The measurement that decides where this crate is useful: the engine's skip is
    /// an order of magnitude faster than the sieve, and its walk is slower. Asserted
    /// of **every** minted machine and every regime it claims, so new silicon either
    /// reproduces the bracket or says out loud that the economics there are different.
    ///
    /// The bracket is what makes residency a dimension rather than a rescaling. It has
    /// to hold in both regimes — a `memchr` that ever became *slower* than the
    /// composition kernel would invert the whole cost model — and yet the width of it
    /// is exactly what a regime changes, since the skip end moves with the memory system
    /// while both other ends do not.
    #[test]
    fn every_minted_machine_brackets_the_sieve_between_skip_and_walk() {
        let mut swept = 0;
        for cal in MINTED {
            let sieve = cal.sieve_per_byte(MAX_CONJUNCTS);
            let (arch, host) = (cal.arch, cal.host);
            assert!(
                sieve < cal.dfa_walk,
                "{arch}: but it does beat a per-byte walk ({host})"
            );
            for at in Residency::ALL {
                if !cal.is_measured(at) {
                    continue;
                }
                swept += 1;
                assert!(
                    cal.dfa_skip[at as usize] < sieve,
                    "{arch} in {at:?}: no per-byte filter can front a memchr ({host})"
                );
            }
        }
        assert!(swept > 0, "no minted row claimed any regime");
    }

    /// A rare lead byte makes the engine unbeatable and a common one makes it barely
    /// better than walking. That ordering is the entire content of the escape-set
    /// model, so it is worth pinning independently of the coefficients — and on every
    /// machine, since it is the ordering that makes anything ever decline.
    #[test]
    fn a_rarer_accelerator_prices_the_rival_cheaper() {
        let freq = prior::Prior::Source.byte_freq();
        for cal in MINTED {
            for at in Residency::ALL {
                if !cal.is_measured(at) {
                    continue;
                }
                let rare = cal.rival_per_byte(b"W", &freq, at);
                let common = cal.rival_per_byte(b"e", &freq, at);
                let none = cal.rival_per_byte(b"", &freq, at);
                let arch = cal.arch;
                assert!(
                    rare < common,
                    "{arch} in {at:?}: a rare escape byte is a cheaper engine: {rare} vs {common}"
                );
                assert!(
                    common <= none,
                    "{arch} in {at:?}: no accelerator can cost more than plain walking"
                );
                assert!(
                    rare < cal.sieve_per_byte(MAX_CONJUNCTS),
                    "{arch} in {at:?}: a rare-anchored engine must out-price the sieve, \
                     or nothing declines"
                );
            }
        }
    }

    /// The direction the regime moves things, pinned so a re-mint cannot quietly invert
    /// it: a cache-resident haystack makes the *engine* cheaper, never the sieve.
    ///
    /// This is the finding that forced the dimension, and it is worth an assertion
    /// rather than only a paragraph. Both regime-indexed coefficients describe reaching
    /// memory — a `memchr` stream and a dense-DFA re-entry — so a row where either grew
    /// on the way into cache would be a row measured through something other than the
    /// memory system, which is the most likely way for a future mint to be wrong.
    #[test]
    fn a_cache_resident_haystack_only_ever_cheapens_the_engine() {
        let (cache, memory) = (Residency::Cache as usize, Residency::Memory as usize);
        let mut compared = 0;
        for cal in MINTED {
            if !(cal.is_measured(Residency::Cache) && cal.is_measured(Residency::Memory)) {
                continue;
            }
            compared += 1;
            let arch = cal.arch;
            assert!(
                cal.dfa_skip[cache] <= cal.dfa_skip[memory],
                "{arch}: a memchr cannot be slower with the bytes already in cache"
            );
            assert!(
                cal.dfa_excursion[cache] <= cal.dfa_excursion[memory],
                "{arch}: re-entering a resident DFA cannot cost more than a cold one"
            );
            // `skip_excursion` is deliberately **not** held to this. Both columns of it
            // re-enter a sixteen-block quotient that is resident in either regime, so
            // the physics predicts no gap — and it is minted as a *maximum* over a
            // five-pattern slate, which is a noisy statistic by construction. Measured,
            // instrument 1 comes out 7.79 in cache against 6.96 in memory: an inversion,
            // and one well inside the spread of the coefficient it belongs to. Asserting
            // a direction there would be asserting the absence of noise.
        }
        assert!(
            compared > 0,
            "no row holds both regimes yet — nothing above was actually compared"
        );
    }
}
