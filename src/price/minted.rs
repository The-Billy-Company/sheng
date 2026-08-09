//! The measured evidence: one [`Calibration`] row per (operating system, architecture,
//! kernel) triple anybody has actually timed, plus the fail-safe row for everyone else.

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
/// The absolute per-byte figures — `dfa_skip`, `dfa_walk`, `sieve` — carry up to
/// double-digit-percent run-to-run variance on a loaded machine, because `mint`
/// times them unpaired. Because the gate is scale-invariant, that variance costs
/// no decisions: a run under load inflates them together.
///
/// A re-mint is a fresh complete measurement, not a splice of old and new
/// afternoons: the gate reads *ratios* between these numbers, and a ratio built
/// from two different sessions is not a measurement of anything. The exceptions are
/// the two excursion coefficients, which are dimensionless and already
/// self-normalized inside a single interleaved timing window — `mint`'s `paired`
/// re-times both baselines against the pattern they divide, round by round — so
/// they may be carried forward when the rest of the row is re-taken. The higher of
/// consecutive paired mints is the one recorded, because an overstated excursion
/// can only decline a skip.
///
/// Which figures move is itself evidence, and worth trusting over any single run.
/// Two independent mints six days and one `regex-automata` removal apart re-derived
/// this row's memory-resident `dfa_excursion` to within a fraction of a percent,
/// while the unpaired figures beside it moved by several — the split falls exactly
/// where `paired` is and is not used. So an excursion coefficient that looks like
/// it drifted has almost certainly been read across regimes rather than across
/// afternoons; see below.
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
    os: "macos",
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
/// per (operating system, architecture, kernel) triple instead of one default.
pub const LINUX_X86_64_SSSE3: Calibration = Calibration {
    os: "linux",
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
    os: "unmeasured",
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

/// Every (operating system, architecture, kernel) triple anybody has actually measured.
/// [`super::active`]
/// picks from here by matching the running target; adding silicon means adding a
/// row, not editing a default.
///
/// The key is `(os, architecture, kernel)`, and the first column is here because a
/// measurement put it here. It used to be `(architecture, kernel)`, on the reasoning
/// that Windows was not a third row waiting to be minted but a claim that the other two
/// already covered it — with the standing condition that an OS column "would only earn
/// its keep the day one leg measures a loss the other two don't".
///
/// That day came the first time `.github/workflows/native.yml` actually ran. Wired into
/// `ci.yml` rather than only `release.yml`, its six Windows/Linux/macOS × x86_64/arm64
/// legs put three machines on a row minted on a fourth, and `examples/survey.rs` caught
/// every one of the three arming a pattern that then lost against real source text —
/// `macos` x86_64 by 3%, both `aarch64` servers by 8%. The mint says why: that macOS box
/// times its own SSSE3 sieve at 0.54 ns/B where the Linux row it was borrowing claims
/// 0.22, while the engine walk it is weighed against differs by only half that. A row is
/// a claim about one machine's memory system, and the three legs that passed had been
/// lucky in their silicon rather than vindicated by it.
///
/// (`os`, `architecture`) is not the *right* key — the right key is the machine, and two
/// `aarch64` Linux servers can differ. It is the finest key a running binary can ask
/// about itself, which is a different and more useful property. See
/// [`OS`](crate::price::OS).
///
/// # This slice is also the dispatch ladder's permission list
///
/// [`crate::shuffle::kernel`] will not select a kernel that has no row here, so what
/// is *absent* from this slice is as load-bearing as what is present. Two consequences
/// worth stating out loud:
///
/// * A new instruction set lands without a flag day. A kernel is implemented,
///   differentially tested against the scalar reference on real silicon by
///   `tests/kernels.rs`, and **not dispatched to** until a row below was measured on it.
///   Adding one arms it; until then it moves no decision, and — the failure this ordering
///   exists to prevent — it cannot win a dispatch on a machine whose only calibration
///   describes a narrower shuffle and thereby strand that install on [`UNMEASURED`].
///   Which kernels are in that state is not left to be inferred from this slice's
///   absences: [`DORMANT`] names them, and a test holds the two lists to each other.
/// * A row is per *kernel* as well as per machine, which is why these names carry all
///   three parts of the key. One `cargo run --release --example mint` prints a row for
///   every kernel the running silicon has, so a machine's rows can be pasted in
///   together or one at a time without either one implying the other.
///   `.github/workflows/mint.yml` is that run, on real hardware, for each of the six
///   machines `.github/workflows/native.yml` proves this crate correct on.
///
/// And what this slice orders is nothing. [`crate::shuffle::kernel`] ranks the rungs a
/// machine has by the `sieve` cost its own rows report, so the *order* of the entries
/// here is presentation only — a slower kernel listed first cannot win a dispatch, and a
/// faster one listed last cannot lose it.
pub const MINTED: &[Calibration] = &[MACOS_AARCH64_NEON, LINUX_X86_64_SSSE3];

/// Kernels this crate implements and some silicon can execute, that [`MINTED`] prices on
/// no architecture — so [`crate::shuffle::kernel`] will not elect them anywhere.
///
/// Empty is the goal, not the invariant. A kernel is written and differentially tested
/// long before anybody has an hour of the right silicon to price it on, and the
/// permission check in [`crate::arch::kernel`] exists precisely so that shipping it in
/// that state is safe rather than reckless. What this list adds is that the state has to
/// be **declared**, with the reason, by whoever leaves it that way.
///
/// The failure it catches is a quiet one, and it is the reason this is a list rather than
/// a paragraph. A kernel nobody has minted yet and a kernel that *stopped* being priced —
/// because a row was deleted, or because a wider rung was added above a priced one and
/// the mint was never re-run — are indistinguishable in [`MINTED`]: both are simply
/// absent. One is a plan and the other is a regression that costs every machine on that
/// silicon its throughput while every test still passes, since the narrower kernel is
/// still correct. Naming the first is what makes the second visible.
///
/// The test below holds this honest in both directions: a vector kernel this silicon can
/// run must be either priced or named here, and a kernel named here must not turn out to
/// be priced after all. So a mint that lands a row also has to delete its line.
pub const DORMANT: &[(Kernel, &str)] = &[
    (
        Kernel::Avx512,
        "differentiated under Intel SDE by `ci.yml`, which proves the kernel and prices \
         nothing; the first mint that reached real silicon read 0.335 ns/B against AVX2's \
         0.290, but that was measured against a kernel spilling its four chains to the \
         stack every step, so it prices nothing either and both have been re-run since",
    ),
    (
        Kernel::Avx2,
        "awaiting the x86_64 leg of `.github/workflows/mint.yml`, which is the first mint \
         this crate can run on real AVX2 silicon",
    ),
    (
        Kernel::Simd128,
        "a `wasm32` row's nanoseconds belong to the runtime and host under it, so the \
         `mint` leg that prices one has to name both",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::Residency;
    use crate::prior;

    /// Every row must describe a machine that could exist, and no two rows may claim
    /// the same (os, architecture, kernel) triple — [`super::super::active`] resolves by
    /// first match, so a duplicate would silently shadow a measurement.
    #[test]
    fn the_minted_rows_are_distinct_and_self_describing() {
        for (i, cal) in MINTED.iter().enumerate() {
            assert!(
                Residency::ALL.iter().any(|&at| cal.is_measured(at)),
                "{} {} row {i} measured nothing in any regime",
                cal.os,
                cal.arch
            );
            assert!(
                cal.kernel.is_vector(),
                "a scalar-kernel row would price the vector economics wrongly"
            );
            assert!(
                MINTED[..i]
                    .iter()
                    .all(|seen| (seen.os, seen.arch, seen.kernel) != (cal.os, cal.arch, cal.kernel)),
                "duplicate calibration for {} {} / {:?}",
                cal.os,
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
        let row = |kernel| {
            MINTED.iter().find(|c| {
                c.os == crate::price::OS && c.arch == crate::price::ARCH && c.kernel == kernel
            })
        };
        if ladder.iter().copied().any(|k| row(k).is_some()) {
            assert!(
                row(chosen).is_some(),
                "{} {} has a priced kernel but dispatch chose the unpriced {chosen:?}",
                crate::price::OS,
                crate::price::ARCH
            );
            // And the *cheapest measured* such, or the rows are decoration. Not the
            // widest: `available` orders by register width, which x86_64 has already
            // measured to be the wrong order, so this reproduces the rule in
            // `arch::kernel` rather than the prior it overrides.
            let best = ladder
                .iter()
                .copied()
                .filter_map(|k| row(k).map(|c| (k, c.sieve_per_byte(MAX_CONJUNCTS))))
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(k, _)| k);
            assert_eq!(
                Some(chosen),
                best,
                "dispatch settled for {chosen:?} while {best:?} is both cheaper and priced"
            );
        }
    }

    /// Every vector kernel this silicon can execute is either priced or **declared**
    /// dormant, and nothing declared dormant is secretly priced.
    ///
    /// The test above proves dispatch never elects an unpriced kernel, which is the
    /// safety property. This one is about the opposite hazard, which is not a safety
    /// property at all and is therefore the easier one to ship: a kernel that silently
    /// stops being dispatched to reads identically to one nobody has written yet, costs
    /// throughput rather than correctness, and breaks no other test in this crate. The
    /// only thing that can catch it is a list somebody has to edit.
    ///
    /// Scoped to machines that have *any* row, which the operating-system column made a
    /// distinction worth drawing. A machine nobody has minted at all is not a missing
    /// row — it is an unminted machine, it declines every pattern by design, and
    /// [`DORMANT`] is the wrong place to say so, since the kernel may be well priced on
    /// the next box over. Refusing to call that state a pass is `examples/survey.rs`'s
    /// job, and `.github/workflows/native.yml` runs it on all six legs. What is left here
    /// is the narrow, quiet failure this list exists for: a machine that *does* have rows
    /// and is missing one for a kernel its own silicon can run.
    #[test]
    fn an_unpriced_kernel_is_declared_rather_than_merely_absent() {
        let mine = || {
            MINTED
                .iter()
                .filter(|cal| cal.os == crate::price::OS && cal.arch == crate::price::ARCH)
        };
        let priced = |kernel| mine().any(|cal| cal.kernel == kernel);
        let declared = |kernel| DORMANT.iter().any(|&(dormant, _)| dormant == kernel);
        if mine().next().is_some() {
            for &kernel in crate::shuffle::available() {
                assert!(
                    !kernel.is_vector() || priced(kernel) || declared(kernel),
                    "{} {} has rows but none for {kernel:?}, which its silicon can run — \
                     mint one, or add it to DORMANT with the reason it is waiting",
                    crate::price::OS,
                    crate::price::ARCH
                );
            }
        }
        for &(kernel, why) in DORMANT {
            assert!(
                !priced(kernel),
                "{kernel:?} is priced on {} {} but still listed DORMANT as {why:?} — a \
                 row landing is what deletes that line",
                crate::price::OS,
                crate::price::ARCH
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
