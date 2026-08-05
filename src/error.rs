//! Why a pattern got no sieve.

use crate::price::{self, CostFact};
use crate::projection::Decline;
use crate::shuffle::Kernel;

/// Why a pattern got no sieve. None of these are failures of the caller's
/// pattern — they mean a per-byte filter would not have paid, or could not have
/// been proven sound, so the caller should simply run its matcher unfiltered.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    /// The pattern did not compile, or its DFA exceeded the build limits.
    Automaton(String),
    /// The automaton's shape rules a sieve out. See [`Decline`].
    Shape(Decline),
    /// No closed partition small enough for a register carried any discriminating
    /// power.
    NoQuotient,
    /// Nobody has measured this machine, so no speedup can be promised on it.
    ///
    /// Not a defect and not a pattern problem: the arming gate compares measured
    /// times, and [`price::MINTED`] holds no row for this (architecture, kernel) pair.
    /// Either mint one (`cargo run --release --example mint`) and pass it in a
    /// [`crate::Policy`], or accept that this machine runs unfiltered. Guessing from
    /// another machine's silicon is the one option deliberately not offered.
    Uncalibrated {
        /// The running target's `std::env::consts::ARCH`, exactly as reported —
        /// what to name when minting the missing row.
        arch: &'static str,
        /// Which kernel this target would dispatch to, had a price been minted for it.
        kernel: Kernel,
    },
    /// A sieve exists and is sound, but fronting this particular engine with it
    /// costs more than letting the engine run alone — so it declines rather than
    /// slow the caller down. The retained arithmetic says why.
    ///
    /// This is the common outcome, and the intended one. Two independent things
    /// cause it: a filter that rejects too little to matter, and a rival cheap
    /// enough that nothing per-byte can front it profitably. The second is why the
    /// decision cannot be a threshold on selectivity alone.
    NotWorthIt(CostFact),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Automaton(why) => write!(f, "no sieve: {why}"),
            Self::Shape(d) => write!(f, "no sieve: {d}"),
            Self::NoQuotient => write!(
                f,
                "no sieve: no register-sized closed partition discriminates"
            ),
            Self::Uncalibrated { arch, kernel } => write!(
                f,
                "no sieve: nothing measured for {arch} with the {kernel:?} kernel — \
                 mint a calibration and pass it in a Policy"
            ),
            Self::NotWorthIt(cost) => write!(
                f,
                "no sieve: {:.3}x — passes {:.2e} of positions, {:.1}% of {:.0}-byte haystacks; \
                 sieve {:.3} ns/B in front of a {:.3} ns/B engine",
                cost.speedup(),
                cost.fallthrough,
                price::survival(cost.fallthrough, cost.len) * 100.0,
                cost.len,
                cost.sieve,
                cost.rival
            ),
        }
    }
}

impl std::error::Error for BuildError {}
