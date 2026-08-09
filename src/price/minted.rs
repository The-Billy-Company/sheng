//! The measured evidence: one [`Calibration`] row per (architecture, kernel) pair
//! anybody has actually timed, plus the fail-safe row for everyone else.

use super::calibration::Calibration;
use crate::lattice::MAX_CONJUNCTS;
use crate::shuffle::Kernel;

/// Minted by `cargo run --release --example mint` over 64 MiB of this repository's
/// real source, each kernel timed alone as the minimum of seven full traversals.
///
/// These numbers state the whole economics of this crate:
///
/// * the sieve runs at **0.188 ns/B**, so it beats the engine's per-byte walk
///   (1.262) by **6.7x** — a real advantage, and the reason any of this pays;
/// * but the engine's *skip* runs at **0.0158 ns/B**, which is still **12x faster
///   than the sieve**. Nothing that inspects every byte can front a `memchr`. That is
///   not a defect in the kernel; it is the arithmetic that decides where it belongs.
///
/// `dfa_excursion` is solved from eleven lead bytes spanning two orders of magnitude
/// of frequency rather than assumed, and the eleven solutions agree within 1.7x
/// (7.26 to 12.12) around the mean of 9.75. They did **not** agree while the escape
/// frequency was read at class resolution — the same eleven bytes then spanned 3.6 to
/// 35.2 — so that 10x spread was the approximation talking, and closing it is what
/// makes a single coefficient defensible here.
///
/// The one-conjunct slot is unmeasured because the lattice harvest fills to
/// [`MAX_CONJUNCTS`] whenever it yields anything at all, so no pattern on the mint's
/// slate reaches it. [`Calibration::sieve_per_byte`] extrapolates it conservatively
/// rather than treating the hole as free.
///
/// Every figure is the minimum of seven full traversals and still carries roughly
/// 10% run-to-run variance — this laptop routinely has ten coworker agents on it, so
/// a re-mint that moves a coefficient by a tenth has not found anything. Because the
/// gate is scale-invariant, that variance costs no decisions: a run under load
/// inflates all four numbers together.
///
/// **Re-minted 2026-08-03** because the kernel changed under it. The old row priced
/// the sieve at 0.514 ns/B, measured when [`crate::shuffle`] held the state in the
/// register and ran one dependent shuffle per byte; holding the transition *function*
/// instead lets four slices compose in parallel and took the same slate to 0.188.
/// Nothing else in this row was meant to move — `dfa_skip`, `dfa_walk` and
/// `dfa_excursion` time `regex-automata`, which did not change — and the ≤8% they
/// drifted is the run-to-run variance above. They are re-taken anyway rather than
/// spliced, because the gate reads *ratios* between these numbers and a ratio built
/// from two different afternoons is not a measurement of anything.
///
/// `skip_excursion` **is** spliced in, and it is the one coefficient here that may
/// be. The rule above exists because a ratio between two absolute ns/B figures is
/// only meaningful when both saw the same machine; this coefficient is not an
/// absolute at all. It is dimensionless and already self-normalized *inside a single
/// interleaved timing window* — `mint`'s `paired` re-times both its baselines against
/// the pattern they divide, round by round — so it carries no dependence on the
/// afternoon it was taken. That is checkable rather than asserted: two consecutive
/// paired mints under load average 12 read `[9.245, 6.398]` and `[9.411, 6.823]`,
/// while the unpaired sweep they replaced read `5.33` and `9.08` for the same
/// instrument on consecutive runs. The higher pair is the one recorded, because an
/// overstated excursion can only decline a skip.
///
/// `dfa_excursion` still carries its pre-pairing value and is the coefficient most
/// likely to move on the next full re-mint: measured unpaired it drifts ~21% run to
/// run on this machine. It is left alone rather than half-corrected, for the
/// two-afternoons reason above — re-mint the row whole.
pub const MACOS_AARCH64_NEON: Calibration = Calibration {
    arch: "aarch64",
    kernel: Kernel::Neon,
    host: "macos aarch64 · 16 logical cores · Neon kernel",
    minted: "2026-08-03",
    dfa_skip: 0.015817,
    dfa_walk: 1.262153,
    dfa_excursion: 9.749545,
    skip_excursion: [9.410733, 6.822599],
    sieve: [0.0, 0.188196],
};

/// Native x86_64 Linux on an idle 13th-gen Intel box, 20 logical cores.
///
/// **Re-minted 2026-08-03**, same day and same procedure as [`MACOS_AARCH64_NEON`], and
/// for the same reason: the prior row (2026-07-29) timed the serial-shuffle kernel,
/// which [`crate::shuffle`] no longer runs — it now composes four slices in parallel
/// against the held transition function. This row supersedes the old one outright
/// rather than adjusting it; every figure below is a fresh, complete, same-machine
/// measurement, not a splice.
///
/// With both rows on the composing kernel, they finally read as a comparison instead
/// of an artifact of two different afternoons. Absolute walk cost is nearly identical
/// (1.252 against arm64's 1.262 ns/B — a dependent-load chain either way), and
/// SSSE3's `memchr` is, if anything, the *cheaper* accelerator relative to its own
/// walk here (skip/walk 1.03% against NEON's 1.25%) — the opposite of what the old,
/// kernel-mismatched row implied. The sieve itself is where the silicon still
/// disagrees: 0.218 ns/B here against 0.188 on arm64, a ~16% gap the shared kernel
/// shape does not erase. Inheriting one machine's numbers on the other would still
/// misprice which patterns arm — which is the whole reason this crate keeps a row
/// per (architecture, kernel) pair instead of one default.
pub const LINUX_X86_64_SSSE3: Calibration = Calibration {
    arch: "x86_64",
    kernel: Kernel::Ssse3,
    host: "linux x86_64 · 20 logical cores · Ssse3 kernel",
    minted: "2026-08-03",
    dfa_skip: 0.012845,
    dfa_walk: 1.251617,
    dfa_excursion: 11.554774,
    skip_excursion: [8.849832, 7.255182],
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
    dfa_skip: 0.0,
    dfa_walk: 0.0,
    dfa_excursion: 0.0,
    skip_excursion: [0.0; 2],
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
    use crate::prior;

    /// Every row must describe a machine that could exist, and no two rows may claim
    /// the same (architecture, kernel) pair — [`super::super::active`] resolves by
    /// first match, so a duplicate would silently shadow a measurement.
    #[test]
    fn the_minted_rows_are_distinct_and_self_describing() {
        for (i, cal) in MINTED.iter().enumerate() {
            assert!(cal.is_measured(), "{} row {i} measured nothing", cal.arch);
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
    /// of **every** minted machine, so new silicon either reproduces the bracket or
    /// says out loud that the economics there are different.
    #[test]
    fn every_minted_machine_brackets_the_sieve_between_skip_and_walk() {
        for cal in MINTED {
            let sieve = cal.sieve_per_byte(MAX_CONJUNCTS);
            let (arch, host) = (cal.arch, cal.host);
            assert!(
                cal.dfa_skip < sieve,
                "{arch}: no per-byte filter can front a memchr ({host})"
            );
            assert!(
                sieve < cal.dfa_walk,
                "{arch}: but it does beat a per-byte walk ({host})"
            );
        }
    }

    /// A rare lead byte makes the engine unbeatable and a common one makes it barely
    /// better than walking. That ordering is the entire content of the escape-set
    /// model, so it is worth pinning independently of the coefficients — and on every
    /// machine, since it is the ordering that makes anything ever decline.
    #[test]
    fn a_rarer_accelerator_prices_the_rival_cheaper() {
        let freq = prior::Prior::Source.byte_freq();
        for cal in MINTED {
            let rare = cal.rival_per_byte(b"W", &freq);
            let common = cal.rival_per_byte(b"e", &freq);
            let none = cal.rival_per_byte(b"", &freq);
            let arch = cal.arch;
            assert!(
                rare < common,
                "{arch}: a rare escape byte is a cheaper engine: {rare} vs {common}"
            );
            assert!(
                common <= none,
                "{arch}: no accelerator can cost more than plain walking"
            );
            assert!(
                rare < cal.sieve_per_byte(MAX_CONJUNCTS),
                "{arch}: a rare-anchored engine must out-price the sieve, or nothing declines"
            );
        }
    }
}
