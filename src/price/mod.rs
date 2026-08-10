//! What each kernel costs, and the one inequality that decides whether a sieve
//! arms.
//!
//! # Both sides measured, neither a ratio
//!
//! A prefilter is worth arming when running it, plus paying on the survivors whatever
//! the caller would otherwise have paid on everything, beats paying it on everything:
//!
//! ```text
//! alternative = min(rivals * rival, bypass)
//! (sieve  +  (1 - (1-f)^len) * alternative) * (1 + MARGIN)   <   alternative
//! ```
//!
//! `rivals` is one for a caller with one pattern, and every clause below reads as if it
//! were. Above one it is the term that makes a **slate** a different economic
//! proposition from a pattern: the pre-pass is paid once and verification once per
//! rival, so `sieve` divides through and stops being what declines a near-parity
//! filter. See [`crate::Policy::rivals`] for the two obligations that keeps honest.
//!
//! `bypass` is the term that keeps the right-hand side a pipeline somebody would really
//! run. A caller whose survivors cost an OCR pass would not put every document through
//! one — they would run the engine first, exactly, for a hundredth of the price — so
//! comparing a sieve against the OCR is comparing it against a strawman, and arms
//! filters that lose to the engine by two orders of magnitude. [`Bypass`] carries the
//! whole argument, including why no true-hit-rate term is needed to make it.
//!
//! What no term can move is [`CostFact::ceiling`]: every amortizing term above works by
//! shrinking the pre-pass, so all of them converge on `1 / survival` and none of them
//! passes it.
//!
//! Every term is an absolute per-byte cost, and [`MARGIN`] is there because every one
//! of them is a *measurement*: a verdict is a ratio of two coefficients whose own
//! run-to-run spread the mint publishes, so a decision inside that spread is a
//! coincidence rather than a finding. A threshold on `f` alone cannot express this,
//! because it has no term for what the rival costs — and when the rival can skip, no
//! per-byte filter can front it profitably however selective it is.
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
//! inequality. So the gate does not depend on this machine's clock, its thermal state,
//! or how many other jobs are on it. That is
//! `scaling_the_whole_calibration_changes_no_decision`, a test rather than a claim.
//!
//! What survives the scaling is three **dimensionless ratios**: skip-to-walk,
//! sieve-to-walk, and the excursion multiplier. Those are what a re-mint on new silicon
//! is actually re-measuring, and [`MINTED`] holds one row per (operating system,
//! architecture, kernel) triple anybody has measured.
//!
//! Scale invariance frees a row from a *clock*, not from a *machine*. Two boxes running
//! the identical kernel price the sieve differently enough to move the arming ratio, so
//! the instruction set fixes what the kernel *is* and the cache hierarchy fixes what it
//! costs against its rival. Hence the machine in the key — see [`MINTED`] for the
//! measurement that put it there, and [`OS`] for what that column can and cannot say.
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
//!
//! # The rival need not be an engine
//!
//! Reading the price off the automaton answers "how long would confirming this pattern
//! take" — which is the right question only when confirming is all a survivor costs. A
//! refutation's actual product is a proof that a document needs **no further work**, and
//! where that work is a network fetch, a model call, or a document extraction, the rival
//! term is orders of magnitude larger than any DFA. [`Rival`] is where a caller states
//! that — and which confirms are and are not expensive enough to bother — while
//! [`crate::Policy::rivals`] is where they state how many such confirms one refutation
//! skips. The two multiply, and both divide the same pre-pass.
//!
//! Naming a large one is not by itself an argument for arming, and that is the part
//! easiest to get wrong. An expensive confirm raises the bar a sieve is measured against
//! only where the caller would genuinely have paid it on every document — and a caller
//! holding a regex almost never would, because the engine decides the same question
//! exactly for a hundredth of the price. [`Bypass`] is the term that asks.

mod calibration;
mod gate;
#[cfg(all(feature = "std", feature = "regex-automata"))]
mod measure;
mod minted;

pub use calibration::{Bypass, Calibration, REGIMES, RESIDENT_ABOVE, Residency, Rival, active};
pub use gate::{CostFact, MARGIN, NOMINAL_LEN, VALIDITY_FLOOR, survival};
#[cfg(all(feature = "std", feature = "regex-automata"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std", feature = "regex-automata"))))]
pub use measure::{
    Bench, Census, MEASURABLE_ABOVE, ROUNDS, Report, Solution, Unmeasurable, histogram,
};
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
