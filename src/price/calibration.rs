//! The `Calibration` shape and the per-byte arithmetic it answers — resolving the
//! running machine's row is [`active`]; the rows themselves live in
//! [`super::minted`].

use super::minted::{MINTED, UNMEASURED};
use crate::lattice::MAX_CONJUNCTS;
use crate::shuffle::{self, Kernel};
use crate::skip::Skip;

/// Per-byte costs for every kernel the gate weighs, each timed **alone** so one
/// coefficient can be re-minted without re-deriving any other.
#[derive(Debug, Clone, Copy)]
pub struct Calibration {
    /// The instruction set these ratios describe, spelled as `std::env::consts::ARCH`
    /// so [`active`] can match it against the running target.
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
    /// effectively `memchr` throughput.
    pub dfa_skip: f64,
    /// The same DFA with no accelerator to skip with: the dependent-load walk.
    pub dfa_walk: f64,
    /// Bytes charged at walk price per escape byte the accelerator trips over.
    ///
    /// An accelerated engine does not pay for one byte when `memchr` finds a
    /// candidate — it enters the DFA, walks a short run, returns, and restarts the
    /// skip, and the restart is most of the cost at this granularity. Without this
    /// term the model under-priced a common-byte accelerator by roughly 8x, which
    /// declined patterns that genuinely paid.
    pub dfa_excursion: f64,
    /// The same quantity for the sieve's own [`crate::Skip`] loop, per instrument.
    ///
    /// A separate number because it is a separate physical event. The engine's
    /// excursion leaves `memchr`, enters a dense DFA whose table does not fit in
    /// L1, walks, and restarts the accelerator; the sieve's enters a sixteen-block
    /// quotient that does, and resumes a probe whose two tables are already in
    /// registers. Measured, the sieve's classifier excursion is roughly 2.3x
    /// cheaper — and charging it the engine's rate declined skips that paid.
    ///
    /// Indexed by [`crate::skip::Instrument`] because the two instruments restart
    /// at genuinely different prices: `memchr` re-enters an aligned multi-stage
    /// loop, the nibble classifier re-enters two registers and a sixteen-byte step.
    pub skip_excursion: [f64; 2],
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
#[must_use]
pub fn active() -> Calibration {
    let kernel = shuffle::kernel();
    MINTED
        .iter()
        .copied()
        .find(|cal| cal.arch == std::env::consts::ARCH && cal.kernel == kernel)
        .unwrap_or(UNMEASURED)
}

impl Calibration {
    /// Was anything here actually timed? A calibration with no sieve measurement can
    /// only decline, and saying so as its own answer beats letting a caller puzzle
    /// over an infinite cost.
    #[must_use]
    pub fn is_measured(&self) -> bool {
        self.sieve.iter().any(|&cost| cost > 0.0) && self.dfa_walk > 0.0
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
            // One doubling per unmeasured step up; exact in binary, so `powf` buys
            // nothing a shift does not.
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
    #[must_use]
    pub fn rival_per_byte(&self, accelerator: &[u8], freq: &[f64; 256]) -> f64 {
        if accelerator.is_empty() {
            return self.dfa_walk;
        }
        let escape: f64 = accelerator
            .iter()
            .map(|&b| freq[usize::from(b)])
            .sum::<f64>()
            .clamp(0.0, 1.0);
        let cost = self.dfa_skip * (1.0 - escape) + self.dfa_walk * escape * self.dfa_excursion;
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
    pub fn skip_per_byte(&self, skip: &Skip, freq: &[f64; 256]) -> f64 {
        let leaves = skip.leaves();
        if leaves.is_empty() {
            return self.dfa_skip;
        }
        let escape: f64 = leaves
            .iter()
            .map(|&b| freq[usize::from(b)])
            .sum::<f64>()
            .clamp(0.0, 1.0);
        let excursion = self.skip_excursion[skip.instrument() as usize];
        let cost = self.dfa_skip * (1.0 - escape) + self.dfa_walk * escape * excursion;
        cost.min(self.dfa_walk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::{CostFact, MACOS_AARCH64, NOMINAL_LEN};
    use crate::prior;

    const UNMINTED: Calibration = UNMEASURED;

    #[test]
    fn a_target_nobody_measured_is_infinite_never_free() {
        for n in 1..=MAX_CONJUNCTS {
            assert!(
                UNMINTED.sieve_per_byte(n).is_infinite(),
                "a zero coefficient would pass every worth test"
            );
        }
        assert!(
            !UNMEASURED.is_measured(),
            "the unmeasured row must admit it is unmeasured"
        );
        assert!(MACOS_AARCH64.is_measured());
    }

    /// The claim that decouples this crate from one laptop: the gate reads three
    /// dimensionless ratios, so scaling every coefficient — a slower clock, a hotter
    /// die, ten coworker agents on the machine — moves no decision at all. Anything
    /// that broke this would make the arming gate a function of ambient load.
    #[test]
    fn scaling_the_whole_calibration_changes_no_decision() {
        let freq = prior::Prior::Source.byte_freq();
        for k in [0.25f64, 1.0, 3.7, 91.0] {
            let scaled = Calibration {
                dfa_skip: MACOS_AARCH64.dfa_skip * k,
                dfa_walk: MACOS_AARCH64.dfa_walk * k,
                // Dimensionless: an excursion is a count of walk-priced bytes, not a
                // duration, so it must NOT scale.
                dfa_excursion: MACOS_AARCH64.dfa_excursion,
                sieve: MACOS_AARCH64.sieve.map(|c| c * k),
                ..MACOS_AARCH64
            };
            for accel in [&b""[..], b"W", b"e", b"abg"] {
                for fallthrough in [0.0, 1e-6, 1e-3, 0.5] {
                    let of = |cal: &Calibration| CostFact {
                        fallthrough,
                        len: NOMINAL_LEN,
                        sieve: cal.sieve_per_byte(MAX_CONJUNCTS),
                        rival: cal.rival_per_byte(accel, &freq),
                    };
                    let (base, now) = (of(&MACOS_AARCH64), of(&scaled));
                    assert_eq!(
                        base.pays(),
                        now.pays(),
                        "k={k} accel={accel:?} f={fallthrough} flipped the gate"
                    );
                    assert!(
                        (base.speedup() - now.speedup()).abs() < 1e-9,
                        "k={k} moved the predicted speedup: {} vs {}",
                        base.speedup(),
                        now.speedup()
                    );
                }
            }
        }
    }

    /// On the machine running the suite, resolution must either find a row minted for
    /// exactly this (architecture, kernel) pair or fall through to the unmeasured one.
    /// Inheriting a foreign row is the failure this pins shut.
    #[test]
    fn resolution_matches_this_machine_or_admits_it_cannot() {
        let cal = active();
        if cal.is_measured() {
            assert_eq!(cal.arch, std::env::consts::ARCH);
            assert_eq!(cal.kernel, crate::shuffle::kernel());
        } else {
            assert!(
                !MINTED
                    .iter()
                    .any(|c| c.arch == std::env::consts::ARCH
                        && c.kernel == crate::shuffle::kernel()),
                "a row exists for this machine but resolution missed it"
            );
        }
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
