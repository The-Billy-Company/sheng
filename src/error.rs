//! Why a pattern got no sieve.

use alloc::string::String;

use crate::price::{self, CostFact};
use crate::projection::Decline;
use crate::shuffle::Kernel;

/// Why a pattern got no sieve. None of these are failures of the caller's
/// pattern — they mean a per-byte filter would not have paid, or could not have
/// been proven sound, so the caller should simply run its matcher unfiltered.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    /// The pattern did not compile, or its DFA exceeded the build limits. Only
    /// reachable through the pattern-string constructors, which are the ones that
    /// own a parser.
    Automaton(String),
    /// The automaton's shape rules a sieve out. See [`Decline`].
    Shape(Decline),
    /// No closed partition small enough for a register carried any discriminating
    /// power.
    NoQuotient,
    /// Nobody has measured this machine, so no speedup can be promised on it.
    ///
    /// Not a defect and not a pattern problem: the arming gate compares measured times,
    /// and [`price::MINTED`] holds no row for this (operating system, architecture,
    /// kernel) triple. Guessing from another machine's silicon is the one option
    /// deliberately not offered.
    ///
    /// # This one the caller can fix from here
    ///
    /// It is the only variant in this enum that a *machine* causes rather than a pattern,
    /// and therefore the only one with a general remedy: measure the machine.
    /// [`Calibration::measure`](price::Calibration::measure) takes a row over the
    /// caller's own documents in a few seconds and
    /// [`Policy::calibration`](crate::Policy::calibration) is where it goes, after which
    /// this variant is unreachable and the gate decides on arithmetic about the machine
    /// that is actually running. `cargo run --release --example mint` is the same
    /// measurement formatted for pasting into [`price::MINTED`], which is what turns one
    /// caller's fix into everybody's.
    ///
    /// A caller who would rather not spend the seconds can accept that this machine runs
    /// unfiltered, which is what happens if this error is ignored — see
    /// [`Screen`](crate::Screen), which ignores it for them.
    ///
    /// All three parts are reported because any two of them are not enough to find the
    /// gap. `MINTED` can hold a row for this architecture *and* this kernel and still not
    /// price this machine — that is the ordinary case for a second operating system on
    /// familiar silicon, and reading only the first two would make the decline look like
    /// a bug in resolution rather than a row nobody has minted yet.
    Uncalibrated {
        /// The running target's [`crate::price::OS`], exactly as reported — the first
        /// third of what to name when minting the missing row.
        os: &'static str,
        /// The running target's [`crate::price::ARCH`], exactly as reported — what
        /// to name when minting the missing row.
        arch: &'static str,
        /// Which kernel this target would dispatch to, had a price been minted for it.
        kernel: Kernel,
    },
    /// The caller says it will search documents shorter than the calibration was
    /// measured over, so a verdict drawn from it would not be about their traffic.
    ///
    /// Distinct from [`Uncalibrated`](Self::Uncalibrated) in what is missing: the row
    /// for this machine exists and was measured honestly, it just does not describe
    /// documents this short. Distinct from [`NotWorthIt`](Self::NotWorthIt) in that no
    /// verdict was reached at all — the terms the model omits would have decided it. See
    /// [`price::VALIDITY_FLOOR`].
    ///
    /// A caller who knows their traffic and wants a filter anyway can say so with
    /// [`Gate::Ungated`](crate::Gate::Ungated), which consults no price. What is not on
    /// offer is a promise this crate has no measurement behind.
    Unmodeled {
        /// The nominal length the caller declared, in bytes.
        len: f64,
        /// [`price::VALIDITY_FLOOR`], the shortest a verdict may be taken at.
        floor: f64,
    },
    /// A sieve exists and is sound, but fronting this particular engine with it
    /// does not measurably beat letting the engine run alone — so it declines rather
    /// than slow the caller down. The retained arithmetic says why.
    ///
    /// This is the common outcome, and the intended one. Three independent things
    /// cause it: a filter that rejects too little to matter, a rival cheap enough
    /// that nothing per-byte can front it profitably, and an edge too thin for the
    /// coefficients it was computed from to resolve ([`price::MARGIN`]). The second
    /// is why the decision cannot be a threshold on selectivity alone; the third is
    /// why it cannot be a threshold on the modeled speedup alone either.
    NotWorthIt(CostFact),
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Automaton(why) => write!(f, "no sieve: {why}"),
            Self::Shape(d) => write!(f, "no sieve: {d}"),
            Self::NoQuotient => write!(
                f,
                "no sieve: no register-sized closed partition discriminates"
            ),
            Self::Uncalibrated { os, arch, kernel } => write!(
                f,
                "no sieve: nothing measured for {os} {arch} with the {kernel:?} kernel — \
                 measure this machine with Calibration::measure and pass the row in a Policy"
            ),
            Self::Unmodeled { len, floor } => write!(
                f,
                "no sieve: a verdict at {len:.0} bytes would be an extrapolation — the \
                 calibration was measured over documents, and holds down to {floor:.0}"
            ),
            Self::NotWorthIt(cost) => write!(
                f,
                "no sieve: {:.3}x, under the {:.2}x a measured decision needs — passes {:.2e} \
                 of positions, {:.1}% of {:.0}-byte haystacks, so {:.2}x is the most any \
                 rival price or slate size could reach; sieve {:.3} ns/B in front of a \
                 {:.3} ns/B engine{}",
                cost.speedup(),
                1.0 + price::MARGIN,
                cost.fallthrough,
                price::survival(cost.fallthrough, cost.len) * 100.0,
                cost.len,
                cost.ceiling(),
                cost.sieve,
                cost.rival,
                // Named only when it is doing something, so the ordinary single-pattern
                // decline reads exactly as it did — but a caller who declared a slate
                // and still declined can see that the fan-out was applied and was not
                // enough, rather than wondering whether it was read at all.
                Fanout(cost.rivals)
            ),
        }
    }
}

impl core::error::Error for BuildError {}

/// The fan-out clause of a [`BuildError::NotWorthIt`] message, which prints nothing at
/// all when there is one rival.
///
/// A `Display` newtype rather than a branch at the call site: the alternative is two
/// nearly identical `write!` arms whose long shared prefix has to be kept in step by
/// hand, which is how a message drifts from the one beside it.
struct Fanout(usize);

impl core::fmt::Display for Fanout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            0 | 1 => Ok(()),
            rivals => write!(f, ", fronting {rivals} of them"),
        }
    }
}
