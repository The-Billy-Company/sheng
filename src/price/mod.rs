//! What each kernel costs, and the one inequality that decides whether a sieve
//! arms.
//!
//! # Both sides measured, neither a ratio
//!
//! A prefilter is worth arming when running it plus verifying whatever survives it
//! beats verifying everything:
//!
//! ```text
//! (sieve  +  (1 - (1-f)^len) * rival) * (1 + MARGIN)   <   rival
//! ```
//!
//! Every term is an absolute per-byte cost, and [`MARGIN`] is there because every one
//! of them is a *measurement*: a verdict is a ratio of two coefficients whose own
//! run-to-run spread the mint publishes, so a decision inside that spread is a
//! coincidence rather than a finding. The version this replaces was a single
//! threshold on `f` alone, which silently assumed three things it could not
//! distinguish: that one conjunct costs what two do, that the sieve's price is a
//! fixed fraction of its rival's, and — worst — that the rival is always a per-byte
//! walk. The third is false on exactly the patterns where it matters most: when the
//! rival can skip, no per-byte filter can front it profitably, and a gate with no
//! term for the rival's price cannot see that.
//!
//! # Nanoseconds, not cycles
//!
//! The inequality is scale-invariant as long as both sides share a unit, so time is
//! measured directly. Converting to cycles would buy nothing and cost one more
//! assumed constant — the clock frequency — which on a modern core is not even
//! fixed for the duration of a scan.
//!
//! # What a measured constant here does and does not pin to one machine
//!
//! Scale invariance is stronger than a convenience: multiply every coefficient in a
//! [`Calibration`] by any positive `k` and [`CostFact::pays`] and
//! [`CostFact::speedup`] are unchanged, because `k` cancels on both sides of the
//! inequality. So the gate does **not** depend on this machine's clock, its thermal
//! state, or how many other jobs are on it — a loaded laptop inflates every
//! coefficient together and decides identically. That is
//! `scaling_the_whole_calibration_changes_no_decision`, a test rather than a claim.
//!
//! What survives the scaling is three **dimensionless ratios**: skip-to-walk,
//! sieve-to-walk, and the excursion multiplier. Those are what a re-mint on new silicon
//! is actually re-measuring, and [`MINTED`] holds one row per (operating system,
//! architecture, kernel) triple anybody has measured.
//!
//! Scale invariance is what frees a row from a *clock*. It does not free one from a
//! *machine*, and this module used to claim otherwise — that the ratios were a property
//! of an instruction set rather than of a serial number. Measurement disagreed: two
//! `x86_64` boxes running the identical SSSE3 kernel price the sieve at 0.22 and
//! 0.54 ns/B, and because the DFA walk they are weighed against differs by only half as
//! much, the ratio that decides arming moves with them. The instruction set fixes what
//! the kernel *is*; the cache hierarchy fixes what it costs against its rival. Hence the
//! machine in the key — see [`OS`].
//!
//! A triple nobody has measured gets [`UNMEASURED`], whose sieve price is infinite, so
//! an unknown machine declines every pattern instead of trusting another machine's
//! numbers. The hazard that makes this fail-closed rather than fussy: the `sieve`
//! coefficients are timed **with a byte shuffle**, and a target without one runs
//! [`crate::shuffle::scalar`] — several times slower — so inheriting a vector
//! measurement there would arm filters that lose.
//!
//! # The rival's price is read from the rival
//!
//! [`Calibration::rival_per_byte`] does not guess whether `regex-automata` will
//! skip; it asks,
//! via `Automaton::accelerator` on the start state, which bytes the engine intends
//! to `memchr` past. Their combined frequency under the prior gives the share of
//! the document the engine walks rather than skips. Underestimating the rival can
//! only make the sieve decline, so the blend deliberately errs that way.

mod calibration;
mod gate;
mod minted;

pub use calibration::{Calibration, REGIMES, Residency, active};
pub use gate::{CostFact, MARGIN, NOMINAL_LEN, VALIDITY_FLOOR, survival};
pub use minted::{
    DORMANT, Dormant, LINUX_AARCH64_NEON, LINUX_X86_64_AVX2, LINUX_X86_64_SSSE3,
    MACOS_AARCH64_NEON, MACOS_X86_64_AVX2, MACOS_X86_64_SSSE3, MINTED, UNMEASURED,
    WINDOWS_AARCH64_NEON, WINDOWS_X86_64_AVX2, WINDOWS_X86_64_AVX512, WINDOWS_X86_64_SSSE3,
};

/// The architecture string [`MINTED`] keys its rows on, and the one a fresh row
/// must name. See [`crate::arch::ARCH`].
pub use crate::arch::ARCH;
/// The operating-system string [`MINTED`] keys its rows on — the other half of the
/// machine half of the key, and the one a fresh row must name. See [`crate::arch::OS`].
pub use crate::arch::OS;
