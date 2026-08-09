//! What a sieve needs from an automaton, and nothing more.
//!
//! A sieve never parses a pattern — it reads a finished DFA, so a pattern means
//! here exactly what it means to the engine that will run the confirming search.
//! That is the crate's whole soundness argument (see [`crate::lattice`]), and it is
//! an argument about *the automaton*, not about whose crate built it.
//!
//! [`Dfa`] is therefore the argument written down: the six questions the projection
//! and the rival's price ask, stated as a contract a caller can satisfy rather than a
//! dependency they must adopt. `regex-automata` satisfies it — `dense::DFA` gets the
//! impl below, behind the default `regex-automata` feature — and so can a hand-built
//! table, a deserialized automaton, or an engine reached over an FFI boundary.
//!
//! # The obligation a caller takes on
//!
//! The quotient over-approximates *this* automaton. So the DFA handed over must be
//! the one that confirms — or one whose language is a superset of it. Feed a sieve
//! a **narrower** automaton than the confirming search and the quotient is no
//! longer a superset of the pattern, and a refutation stops being a proof. Nothing
//! here can check that, which is why the shipped impl reads the real engine's own
//! `Automaton` surface rather than reconstructing anything from a pattern string.

/// A byte-oriented deterministic automaton a sieve can be built from.
///
/// Six methods, all of them things a DFA already knows — no state enumeration, no
/// transition table, nothing about the pattern. [`crate::Projection`] discovers the
/// reachable core by breadth-first search over all 256 bytes, so walkable suffices.
pub trait Dfa {
    /// Whatever names a state. Opaque to this crate; [`Ord`] only so the projection can
    /// intern ids by binary search — which is both the weaker thing to ask of a caller's
    /// type than [`Hash`](core::hash::Hash) plus a hasher, and, at 96 states, the faster
    /// one.
    type State: Copy + Ord;

    /// The unanchored start state for a forward search, or `None` when this
    /// automaton cannot say — which every caller here reads as "decline".
    fn start(&self) -> Option<Self::State>;

    /// The state reached from `state` on `byte`.
    fn next(&self, state: Self::State, byte: u8) -> Self::State;

    /// The state reached from `state` at end of input, which is where a `$`- or
    /// `\z`-anchored pattern's acceptance shows up.
    fn next_eoi(&self, state: Self::State) -> Self::State;

    /// Does a run that has reached `state` have a match in hand?
    fn is_match(&self, state: Self::State) -> bool;

    /// Can this state abandon the search mid-scan? A sieve declines any automaton
    /// that reaches one ([`crate::Decline::Quits`]), because a continuous run does
    /// not model quitting and guessing would be unsound.
    fn is_quit(&self, state: Self::State) -> bool;

    /// The byte values this automaton will skip past while sitting in `state` — what
    /// [`crate::price::Calibration::rival_per_byte`] prices the confirming engine from.
    ///
    /// Deliberately **not** defaulted: the two answers err in opposite directions and
    /// only one is safe to inherit. An empty slice claims the engine walks every byte,
    /// which prices it at its most expensive and makes a sieve *more* likely to arm — so
    /// an implementor whose engine does accelerate has to say so, rather than getting the
    /// optimistic answer by not writing a method.
    fn accelerator(&self, state: Self::State) -> &[u8];
}

/// [`Dfa`] for `regex-automata`'s own dense DFA — the impl that makes the filter and the
/// confirming search provably the same automaton.
///
/// Written for `dense::DFA<T>` over any `T: AsRef<[u32]>` rather than as a blanket impl
/// over `Automaton`, which would claim every implementing type and leave coherence
/// forbidding a caller from implementing [`Dfa`] for their own type at all. This covers
/// both the built `DFA<Vec<u32>>` and the zero-copy deserialized `DFA<&[u32]>`, which is
/// how a sieve gets armed here off an automaton compiled somewhere else entirely.
#[cfg(feature = "regex-automata")]
mod automata {
    use regex_automata::Input;
    use regex_automata::dfa::{Automaton, dense};
    use regex_automata::util::primitives::StateID;

    impl<T: AsRef<[u32]>> super::Dfa for dense::DFA<T> {
        type State = StateID;

        fn start(&self) -> Option<StateID> {
            // An empty haystack asks only for the start configuration; the error
            // case is a start state the DFA declines to give, which is `Quits`.
            self.start_state_forward(&Input::new(b"")).ok()
        }

        fn next(&self, state: StateID, byte: u8) -> StateID {
            self.next_state(state, byte)
        }

        fn next_eoi(&self, state: StateID) -> StateID {
            self.next_eoi_state(state)
        }

        fn is_match(&self, state: StateID) -> bool {
            self.is_match_state(state)
        }

        fn is_quit(&self, state: StateID) -> bool {
            self.is_quit_state(state)
        }

        fn accelerator(&self, state: StateID) -> &[u8] {
            Automaton::accelerator(self, state)
        }
    }
}
