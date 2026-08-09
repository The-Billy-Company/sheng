//! The `Calibration` shape and the per-byte arithmetic it answers — resolving the
//! running machine's row is [`active`]; the rows themselves live in
//! [`super::minted`].

use super::minted::{MINTED, UNMEASURED};
use crate::arch::ARCH;
use crate::lattice::MAX_CONJUNCTS;
use crate::shuffle::{self, Kernel};
use crate::skip::Skip;

/// Where the bytes a caller is about to search are coming from.
///
/// A per-byte price is only a price against a particular memory system, and this is
/// the one fact about the caller's scan that no amount of arithmetic can recover from
/// the pattern. It has no default: see [`crate::Policy::new`].
///
/// # Why this had to become a dimension
///
/// [`Calibration::rival_per_byte`] caps the engine's price at [`Calibration::dfa_walk`],
/// and that cap is what decides whether a pattern is exposed to this at all. A rival
/// with a *frequent* escape set is pinned at the cap — a dependent-load walk, bound by
/// L1 latency and indifferent to where the haystack lives — so the sieve's advantage
/// over it holds in every regime. A rival with a *rare* escape set instead rides
/// [`Calibration::dfa_skip`], which in the memory-resident regime is pinned by DRAM
/// bandwidth and in the cache-resident regime is not pinned by anything the mint
/// measured.
///
/// The gap is not academic. `panic!\(` can price as a clear win over a large
/// memory-resident corpus and measure as a clear loss over a cache-resident one —
/// same pattern, same machine, same coefficients. The engine's accelerated path
/// moves with residency because its excursion re-enters a dense DFA whose
/// transition table misses cache in one regime and hits it in the other. The
/// sieve's own cost barely moves, which is exactly why a single row could not
/// describe both: the comparison is between a term that moves and a term that
/// does not.
///
/// So the uncomfortable half of the finding, stated where a caller will read it: the
/// sieve's edge over an accelerated engine comes substantially from *that engine
/// missing cache*. Remove the memory pressure and the edge shrinks rather than merely
/// scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Residency {
    /// The bytes are already in cache when the scan reaches them — a working set that
    /// fits in last-level cache, or one scanned repeatedly. The regime in which the
    /// engine's `memchr` is at its cheapest and the sieve therefore at its least
    /// competitive.
    Cache = 0,
    /// The bytes are being read from main memory: a corpus larger than last-level
    /// cache, traversed once. The regime the shipped rows were originally minted over.
    Memory = 1,
}

/// How many regimes a regime-indexed coefficient carries. Not a tuning knob — it is
/// the variant count of [`Residency`], and the array indexing below depends on it.
pub const REGIMES: usize = 2;

impl Residency {
    /// Every regime, in coefficient-index order — so a mint can emit each column and a
    /// test can sweep them without either restating the variant list and drifting.
    pub const ALL: [Self; REGIMES] = [Self::Cache, Self::Memory];
}

/// Per-byte costs for every kernel the gate weighs, each timed **alone** so one
/// coefficient can be re-minted without re-deriving any other.
///
/// # Which coefficients carry a regime, and which cannot
///
/// Two of these are indexed by [`Residency`] and two are not, and the split is a
/// claim about what each loop is bound by rather than a convenience:
///
/// * [`Calibration::dfa_skip`] and the excursion coefficients **are** regime-indexed.
///   A `memchr` stream is bandwidth-bound, and an excursion's dominant cost is
///   re-entering a transition table that may or may not be resident.
/// * [`Calibration::dfa_walk`] is **not**. A dependent-load DFA walk waits on L1
///   latency for a table it has already pulled in, one state at a time, and measures
///   nearly the same on both architectures — the same number in both regimes for
///   the same reason it is nearly the same number on both machines.
/// * [`Calibration::sieve`] is **not**. The composition kernel is issue-bound at three
///   operations a byte and runs an order of magnitude under the bandwidth a
///   `memchr` saturates, so it has no headroom to gain from a hotter haystack.
///
/// Keeping them in one row rather than shipping two rows per machine is deliberate.
/// A row is a claim about an (architecture, kernel) pair, and the invariance above is
/// then a structural fact the type enforces instead of a coincidence two independently
/// pasted rows would have to be trusted to preserve.
#[derive(Debug, Clone, Copy)]
pub struct Calibration {
    /// The instruction set these ratios describe, spelled as [`ARCH`] spells it so
    /// [`active`] can match it against the running target.
    pub arch: &'static str,
    /// The kernel the `sieve` coefficients were timed with. A number measured through
    /// a byte shuffle is not a number for a target that has none, which is why this
    /// is matched and not assumed.
    pub kernel: Kernel,
    /// The silicon that produced these numbers, and when. A measured value with no
    /// machine beside it is an anecdote.
    pub host: &'static str,
    /// The date this row was measured, `YYYY-MM-DD`. What ages a row into stale.
    pub minted: &'static str,
    /// `regex-automata`'s dense DFA over bytes its start-state accelerator skips —
    /// effectively `memchr` throughput. Indexed by [`Residency`], because a byte
    /// scan this fast is bound by how quickly bytes arrive rather than by what it
    /// does with them: out of memory it saturates single-core DRAM bandwidth, which
    /// is not a property of the loop.
    pub dfa_skip: [f64; REGIMES],
    /// The same DFA with no accelerator to skip with: the dependent-load walk. Not
    /// regime-indexed — see the type's own documentation.
    pub dfa_walk: f64,
    /// Bytes charged at walk price per escape byte the accelerator trips over.
    ///
    /// An accelerated engine does not pay for one byte when `memchr` finds a
    /// candidate — it enters the DFA, walks a short run, returns, and restarts the
    /// skip, and the restart is most of the cost at this granularity. Without this
    /// term the model under-priced a common-byte accelerator by nearly an order of
    /// magnitude, which declined patterns that genuinely paid.
    ///
    /// Indexed by [`Residency`] because the re-entry is exactly where the memory
    /// system shows up: the table the excursion walks into is the engine's *dense*
    /// DFA, which is far too large for L1, so whether it is otherwise resident
    /// changes the escape cost by about a factor of two.
    pub dfa_excursion: [f64; REGIMES],
    /// The same quantity for the sieve's own [`crate::Skip`] loop, per instrument and
    /// per regime.
    ///
    /// A separate number because it is a separate physical event. The engine's
    /// excursion leaves `memchr`, enters a dense DFA whose table does not fit in
    /// L1, walks, and restarts the accelerator; the sieve's enters a sixteen-block
    /// quotient that does, and resumes a probe whose two tables are already in
    /// registers. Measured, the sieve's classifier excursion is a few times
    /// cheaper — and charging it the engine's rate declined skips that paid.
    ///
    /// The outer index is [`crate::skip::Instrument`], because the instruments restart
    /// at genuinely different prices: `memchr` re-enters an aligned multi-stage loop,
    /// the nibble classifier re-enters two registers and a sixteen-byte step. The
    /// inner index is [`Residency`], for the same reason `dfa_excursion` carries one —
    /// though the sieve's sixteen-byte table is resident in *either* regime, so this is
    /// the coefficient expected to move least.
    pub skip_excursion: [[f64; REGIMES]; 2],
    /// The sieve's own cost, indexed by conjunct count minus one. A zero means
    /// **never measured**, which [`Calibration::sieve_per_byte`] reports as
    /// infinity — a free
    /// pre-pass would pass every worth test.
    pub sieve: [f64; MAX_CONJUNCTS],
}

/// The calibration for the machine that is running, or [`UNMEASURED`].
///
/// Keyed on the architecture **and** the kernel that dispatch actually chose, because
/// an `x86_64` without SSSE3 runs the scalar path and has no business inheriting a
/// `pshufb` measurement. Resolved at run time rather than by `cfg` for the same
/// reason: the kernel is a runtime probe on x86, so a compile-time answer could be
/// wrong on the machine that ends up executing.
///
/// `residency` is **not** part of that key, and the asymmetry is the point. Which
/// silicon is running is a fact this crate can determine; which memory regime the
/// caller's scan is in is not, so it is asked for rather than probed. A row carries
/// both regimes ([`Calibration`]), so what this returns is the whole row and the regime
/// only selects which of its columns the gate reads — except that a row holding no
/// measurement for the regime asked about resolves to [`UNMEASURED`] rather than
/// answering out of the other one.
#[must_use]
pub fn active(residency: Residency) -> Calibration {
    let kernel = shuffle::kernel();
    MINTED
        .iter()
        .copied()
        .find(|cal| cal.arch == ARCH && cal.kernel == kernel && cal.is_measured(residency))
        .unwrap_or(UNMEASURED)
}

impl Calibration {
    /// Was anything here actually timed, *for the regime being asked about*?
    ///
    /// Regime-aware because a row can honestly hold one regime and not the other — a
    /// machine whose memory-resident coefficients were pasted in before its
    /// cache-resident ones were taken is a real state, and it has to read as
    /// uncalibrated for the regime it cannot price rather than borrow the one it can.
    /// That is the same refusal [`UNMEASURED`] makes about a whole machine, applied one
    /// column in.
    #[must_use]
    pub fn is_measured(&self, residency: Residency) -> bool {
        self.sieve.iter().any(|&cost| cost > 0.0)
            && self.dfa_walk > 0.0
            && self.dfa_skip[residency as usize] > 0.0
    }

    /// The sieve's per-byte price at `conjuncts`.
    ///
    /// A count nobody minted is extrapolated from the nearest one that was, and the
    /// direction decides how — both ways erring high, so an unmeasured slot can only
    /// make a sieve decline:
    ///
    /// * **Upward** (want more conjuncts than were measured): double per step. Each
    ///   conjunct is an independent pass over the same bytes, so twice the cost of
    ///   `n` is a sound ceiling for `n+1`.
    /// * **Downward** (want fewer): take the measurement unchanged. Fewer passes
    ///   cannot cost more than more passes, so a higher count's price is already an
    ///   upper bound — and pricing it lower would credit the short-circuit that
    ///   [`crate::Sieve::refutes`] only sometimes gets.
    ///
    /// With nothing measured at all the answer is infinity, never zero: a free
    /// pre-pass passes every worth test.
    #[must_use]
    pub fn sieve_per_byte(&self, conjuncts: usize) -> f64 {
        let want = conjuncts.clamp(1, MAX_CONJUNCTS) - 1;
        if let Some(below) = (0..=want).rev().find(|&i| self.sieve[i] > 0.0) {
            // One doubling per unmeasured step up. A power of two is exact in binary,
            // so a shift is the whole exponentiation.
            return self.sieve[below] * (1u64 << (want - below)) as f64;
        }
        self.sieve[want..]
            .iter()
            .copied()
            .find(|&cost| cost > 0.0)
            .unwrap_or(f64::INFINITY)
    }

    /// What the confirming engine costs per byte, given the bytes it told us it will
    /// skip.
    ///
    /// `accelerator` is `Automaton::accelerator` for the engine's start state: empty
    /// when the engine has no skip and is committed to a walk.
    ///
    /// Otherwise the engine skips most bytes and pays an excursion for each escape
    /// byte the prior expects, which is what makes a **rare** lead byte unbeatable
    /// and a **common** one barely an advantage at all. The result is capped at the
    /// unaccelerated walk: an accelerator that trips on everything degenerates to
    /// walking, never to something slower.
    ///
    /// That cap is also what decides whether this pattern is exposed to `residency` at
    /// all. A frequent escape set pins the answer at [`Calibration::dfa_walk`], which
    /// carries no regime; a rare one rides [`Calibration::dfa_skip`], which carries
    /// the whole of it. See [`Residency`].
    #[must_use]
    pub fn rival_per_byte(&self, accelerator: &[u8], freq: &[f64; 256], at: Residency) -> f64 {
        if accelerator.is_empty() {
            return self.dfa_walk;
        }
        let escape = share(accelerator, freq);
        let cost = self.dfa_skip[at as usize] * (1.0 - escape)
            + self.dfa_walk * escape * self.dfa_excursion[at as usize];
        cost.min(self.dfa_walk)
    }

    /// What the sieve's own [`crate::Skip`] loop costs per byte on this machine.
    ///
    /// The same blend as [`Calibration::rival_per_byte`] and for the same reason —
    /// a skip loop *is* an accelerated DFA, so there is one shape of arithmetic here,
    /// not two. What differs is the excursion coefficient, which is the instrument's
    /// own rather than the engine's.
    ///
    /// A block nothing leaves is the case the blend cannot state: the loop returns
    /// without reading a byte, so it is charged the cheapest coefficient the machine
    /// has rather than nothing at all.
    #[must_use]
    pub fn skip_per_byte(&self, skip: &Skip, freq: &[f64; 256], at: Residency) -> f64 {
        let leaves = skip.leaves();
        if leaves.is_empty() {
            return self.dfa_skip[at as usize];
        }
        let escape = share(leaves, freq);
        let excursion = self.skip_excursion[skip.instrument() as usize][at as usize];
        let cost = self.dfa_skip[at as usize] * (1.0 - escape) + self.dfa_walk * escape * excursion;
        cost.min(self.dfa_walk)
    }
}

/// What share of the corpus a byte set covers under `freq`, clamped to a probability.
///
/// Factored out because both blends above need exactly it, and a set that summed past
/// one — a caller's own marginals need not be normalized — would make an escape rate
/// into a negative residency.
fn share(set: &[u8], freq: &[f64; 256]) -> f64 {
    set.iter()
        .map(|&b| freq[usize::from(b)])
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::{CostFact, MACOS_AARCH64_NEON, NOMINAL_LEN};
    use crate::prior;

    const UNMINTED: Calibration = UNMEASURED;
    const REGIME: [Residency; REGIMES] = Residency::ALL;

    #[test]
    fn a_target_nobody_measured_is_infinite_never_free() {
        for n in 1..=MAX_CONJUNCTS {
            assert!(
                UNMINTED.sieve_per_byte(n).is_infinite(),
                "a zero coefficient would pass every worth test"
            );
        }
        for at in REGIME {
            assert!(
                !UNMEASURED.is_measured(at),
                "the unmeasured row must admit it is unmeasured, in every regime"
            );
        }
        assert!(MACOS_AARCH64_NEON.is_measured(Residency::Memory));
    }

    /// The regime column is a key, not a hint: a row measured in one regime and not the
    /// other must read as uncalibrated for the one it cannot price, and [`active`] must
    /// hand back [`UNMEASURED`] rather than the column it does have.
    ///
    /// This is the same refusal the crate already makes about a whole machine — "no row
    /// for this silicon, so no promise" — pushed one column in, and it is what keeps a
    /// half-minted row from quietly pricing a cache-resident scan off memory-resident
    /// numbers that are 2x too generous.
    #[test]
    fn a_regime_nobody_measured_declines_instead_of_borrowing_the_other() {
        let half = Calibration {
            dfa_skip: [0.0, 0.015817],
            ..MACOS_AARCH64_NEON
        };
        assert!(half.is_measured(Residency::Memory));
        assert!(
            !half.is_measured(Residency::Cache),
            "an unminted regime must not inherit the minted one"
        );
        // And the shipped rows are consistent with themselves: any regime a row claims
        // to have measured must have every coefficient that regime needs.
        for cal in MINTED {
            for at in REGIME {
                if cal.is_measured(at) {
                    assert!(
                        cal.dfa_excursion[at as usize] > 0.0,
                        "{} claims {at:?} with no excursion coefficient",
                        cal.arch
                    );
                    assert!(
                        cal.skip_excursion.iter().all(|per| per[at as usize] > 0.0),
                        "{} claims {at:?} with an unpriced instrument",
                        cal.arch
                    );
                }
            }
        }
    }

    /// The claim that decouples this crate from one laptop: the gate reads three
    /// dimensionless ratios, so scaling every coefficient — a slower clock, a hotter
    /// die, ten coworker agents on the machine — moves no decision at all. Anything
    /// that broke this would make the arming gate a function of ambient load.
    ///
    /// Swept over every regime the row claims, because the invariance has to hold
    /// *within* a regime and the residency axis is precisely the part of the variation
    /// that scaling does **not** cover. A uniform factor is what a clock or a thermal
    /// state does to a machine; moving a haystack from DRAM into cache rescales two
    /// coefficients and leaves two alone, which is why it needed a dimension instead of
    /// being absorbed here.
    #[test]
    fn scaling_the_whole_calibration_changes_no_decision() {
        let freq = prior::Prior::Source.byte_freq();
        let mut swept = 0;
        for at in REGIME {
            if !MACOS_AARCH64_NEON.is_measured(at) {
                continue;
            }
            swept += 1;
            for k in [0.25f64, 1.0, 3.7, 91.0] {
                let scaled = Calibration {
                    dfa_skip: MACOS_AARCH64_NEON.dfa_skip.map(|c| c * k),
                    dfa_walk: MACOS_AARCH64_NEON.dfa_walk * k,
                    // Dimensionless: an excursion is a count of walk-priced bytes, not
                    // a duration, so it must NOT scale.
                    dfa_excursion: MACOS_AARCH64_NEON.dfa_excursion,
                    sieve: MACOS_AARCH64_NEON.sieve.map(|c| c * k),
                    ..MACOS_AARCH64_NEON
                };
                for accel in [&b""[..], b"W", b"e", b"abg"] {
                    for fallthrough in [0.0, 1e-6, 1e-3, 0.5] {
                        let of = |cal: &Calibration| CostFact {
                            fallthrough,
                            len: NOMINAL_LEN,
                            sieve: cal.sieve_per_byte(MAX_CONJUNCTS),
                            rival: cal.rival_per_byte(accel, &freq, at),
                        };
                        let (base, now) = (of(&MACOS_AARCH64_NEON), of(&scaled));
                        assert_eq!(
                            base.pays(),
                            now.pays(),
                            "k={k} at={at:?} accel={accel:?} f={fallthrough} flipped the gate"
                        );
                        assert!(
                            (base.speedup() - now.speedup()).abs() < 1e-9,
                            "k={k} at={at:?} moved the predicted speedup: {} vs {}",
                            base.speedup(),
                            now.speedup()
                        );
                    }
                }
            }
        }
        assert!(swept > 0, "the reference row measured no regime at all");
    }

    /// On the machine running the suite, resolution must either find a row minted for
    /// exactly this (architecture, kernel) pair or fall through to the unmeasured one.
    /// Inheriting a foreign row is the failure this pins shut.
    #[test]
    fn resolution_matches_this_machine_or_admits_it_cannot() {
        for at in REGIME {
            let cal = active(at);
            if cal.is_measured(at) {
                assert_eq!(cal.arch, ARCH);
                assert_eq!(cal.kernel, crate::shuffle::kernel());
            } else {
                assert!(
                    !MINTED.iter().any(|c| c.arch == ARCH
                        && c.kernel == crate::shuffle::kernel()
                        && c.is_measured(at)),
                    "a row exists for this machine in {at:?} but resolution missed it"
                );
            }
        }
    }

    /// [`ARCH`] replaced `std::env::consts::ARCH`, and the replacement is only
    /// correct if it is the *same string* — a mismatch would resolve every machine to
    /// [`UNMEASURED`] and silently disarm the crate rather than fail loudly. Checked
    /// against `std` wherever there is a `std` to check against.
    #[cfg(feature = "std")]
    #[test]
    fn the_cfg_derived_arch_is_the_one_the_standard_library_reports() {
        assert_eq!(ARCH, std::env::consts::ARCH);
    }

    #[test]
    fn extrapolation_errs_high_in_both_directions() {
        // Measured at two conjuncts only — the shape ACTIVE actually ships.
        let cal = Calibration {
            sieve: [0.0, 0.5],
            ..UNMINTED
        };
        // Downward: never cheaper than the count that was measured.
        assert_eq!(cal.sieve_per_byte(1), 0.5);
        assert_eq!(cal.sieve_per_byte(2), 0.5);

        // Upward: each further pass doubles.
        let cal = Calibration {
            sieve: [0.5, 0.0],
            ..UNMINTED
        };
        assert_eq!(cal.sieve_per_byte(1), 0.5);
        assert_eq!(cal.sieve_per_byte(2), 1.0);
    }
}
