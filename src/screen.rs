//! The front door that cannot decline: a matcher that uses a sieve when one pays and
//! the engine alone when one does not.
//!
//! # Why the crate needed one
//!
//! [`Sieve`] hands back a `Result` whose *common* variant is refusal, and that is
//! honest — most patterns should not be fronted by a filter. But it makes the ordinary
//! outcome of `cargo add sheng` an error a caller has to write code around, for a
//! speedup they then do not get, and the rational response to that is to remove the
//! dependency. Which is the wrong outcome twice over: the decline is a fact about *this*
//! pattern on *this* machine over documents of *this* length, and every one of those can
//! change without the caller doing anything. A slate grows. A machine gets a minted row.
//! A corpus moves from cache to memory.
//!
//! So [`Screen`] absorbs the decline instead of reporting it. It builds the pattern's
//! automaton, asks for a sieve, and keeps whichever answer it got:
//!
//! * a sieve armed — refute first, and only run the engine on survivors;
//! * no sieve — run the engine, exactly as if this crate were not in the graph.
//!
//! The only error it can return is an unparseable pattern, which is a caller mistake
//! rather than an economic verdict. Nothing is hidden: [`Screen::sieve`] hands over the
//! sieve and its arithmetic, and [`Screen::declined`] hands over the refusal verbatim.
//! What is removed is the obligation to *handle* it.
//!
//! # What it is not
//!
//! Not a regex engine, and not a replacement for one. `is_match` is the whole surface,
//! because "does a match exist anywhere in these bytes" is the only question a
//! refutation can answer more cheaply than the engine — a sieve never says where a match
//! is. Anything positional belongs to
//! [regex-automata](https://docs.rs/regex-automata) directly.

use alloc::vec::Vec;

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};

use crate::price::Residency;
use crate::{BuildError, Policy, Sieve};

/// A pattern, its automaton, and a sieve if one paid — never slower than the engine
/// alone, and never a decline the caller has to handle.
///
/// One automaton serves both roles, which is the arrangement [`Sieve::of_dfa`]
/// recommends and this type makes automatic: the sieve is priced against the very
/// engine that will confirm its survivors, so the gate's rival term is a measurement of
/// the search that is really going to run rather than of a search like it.
///
/// Immutable and shareable for the same reason a [`Sieve`] is — no scan state, so one
/// instance serves every document and every thread.
///
/// ```
/// # #[cfg(feature = "regex-automata")] {
/// use sheng::{Residency, Screen};
///
/// let screen = Screen::new(r"(?-u)[0-9]{3}-[0-9]{2}-[0-9]{4}", Residency::Memory)
///     .expect("the pattern parses");
/// assert!(screen.is_match(b"user 123-45-6789 flagged"));
/// assert!(!screen.is_match(b"nothing to see here"));
/// # }
/// ```
#[derive(Debug)]
pub struct Screen {
    dfa: dense::DFA<Vec<u32>>,
    /// The verdict, kept whole rather than reduced to an `Option`: a caller who wants to
    /// know *why* they are running unfiltered can read the refusal that says so, with
    /// the arithmetic still attached.
    sieve: Result<Sieve, BuildError>,
}

impl Screen {
    /// Build a screen for `pattern`, filtered if a filter pays and unfiltered if not.
    ///
    /// The only `Err` is [`BuildError::Automaton`] — a pattern that does not parse or
    /// whose automaton exceeded a build limit. Every other refusal is absorbed.
    pub fn new(pattern: &str, residency: Residency) -> Result<Self, BuildError> {
        Self::with(pattern, &Policy::new(residency))
    }

    /// [`Screen::new`] under a caller-supplied [`Policy`] — the seam for a slate
    /// ([`Policy::rivals`]), a document length, or a machine this crate never measured.
    pub fn with(pattern: &str, policy: &Policy<'_>) -> Result<Self, BuildError> {
        let dfa = crate::relax::strict(pattern)?;
        let sieve = Sieve::of_pattern(pattern, &dfa, policy);
        // An unparseable pattern is the caller's problem and was already returned
        // above; an automaton that this crate could build but could not *sieve* is not.
        // The distinction is the whole contract of this type, so it is asserted here
        // rather than left to the reader: nothing that reaches this point may be an
        // `Automaton` error.
        debug_assert!(
            !matches!(sieve, Err(BuildError::Automaton(_))),
            "a pattern that built an automaton reported an automaton error"
        );
        Ok(Self { dfa, sieve })
    }

    /// Does `haystack` contain a match?
    ///
    /// Identical in answer to running the engine alone — that is the promise, and
    /// `tests/screen.rs` holds it to a differential against exactly that. What differs
    /// is the work: when a sieve armed and refutes, the engine never sees the bytes.
    #[must_use]
    pub fn is_match(&self, haystack: &[u8]) -> bool {
        if self.refutes(haystack) {
            return false;
        }
        self.dfa
            .try_search_fwd(&Input::new(haystack))
            .is_ok_and(|found| found.is_some())
    }

    /// Does the sieve **prove** `haystack` holds no match?
    ///
    /// `false` when no sieve armed, which is the honest answer: an absent filter has
    /// refuted nothing. So this is safe to branch on directly for a caller who wants to
    /// run their own confirming search rather than [`Screen::is_match`].
    #[must_use]
    pub fn refutes(&self, haystack: &[u8]) -> bool {
        self.sieve
            .as_ref()
            .is_ok_and(|sieve| sieve.refutes(haystack))
    }

    /// Is a sieve actually in front of the engine?
    #[must_use]
    pub fn armed(&self) -> bool {
        self.sieve.is_ok()
    }

    /// The armed sieve, for its [`Sieve::cost`] and its diagnostics.
    #[must_use]
    pub fn sieve(&self) -> Option<&Sieve> {
        self.sieve.as_ref().ok()
    }

    /// Why there is no sieve, verbatim — including the arithmetic, when the reason was
    /// economic. Absorbing a refusal is not the same as discarding it.
    #[must_use]
    pub fn declined(&self) -> Option<&BuildError> {
        self.sieve.as_ref().err()
    }

    /// The automaton the confirming search runs, so a caller who needs a *position*
    /// rather than an existence answer can ask it directly instead of building a second
    /// copy of their own pattern.
    #[must_use]
    pub fn dfa(&self) -> &dense::DFA<Vec<u32>> {
        &self.dfa
    }
}
