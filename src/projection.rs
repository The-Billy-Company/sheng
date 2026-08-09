//! The projection: what a sieve is allowed to reason about, read off a real DFA.
//!
//! A sieve never parses a pattern. It walks a finished automaton through the six
//! questions [`Dfa`] asks — start, step, step-at-end-of-input, accepts, quits,
//! accelerates — so a pattern means here exactly what it means to the engine that
//! will run the confirming search. There is no second parser to drift.
//!
//! Two things come out of the walk that no DFA hands over directly.
//!
//! **The reachable core.** [`Dfa`] deliberately exposes no state enumeration, so
//! the states are discovered by breadth-first search from the unanchored start over
//! all 256 bytes, compacted into dense `u16` ids. That is not a workaround — it
//! is the only projection that is *true*, because a state no byte string can
//! reach cannot affect whether a document matches. It is also what keeps the trait
//! small enough for a caller to implement over an automaton they did not build.
//!
//! **A minimal byte-class partition.** Two bytes are equivalent when no core
//! state routes them differently. `regex-automata` computes something coarser on
//! purpose — its `ByteClassSet` says so in a comment: it "does not compute the
//! minimal set of equivalence classes", because it partitions contiguous byte
//! *ranges* rather than grouping by transition set. Since the search above
//! already touched all 256 bytes on every core state, the exact partition is
//! free here, and it is what makes the lattice closure below affordable: the
//! closure loop runs once per class, not once per byte.

use alloc::vec;
use alloc::vec::Vec;

use crate::dfa::Dfa;

/// Cost policy on the harvest, not a bound on correctness. Pair closure is
/// `O(n²)` closures of `O(n · classes)` each, so a core wider than this would
/// spend more time being analyzed than the scan it accelerates could ever
/// return. Above it the sieve declines. Mirrors `max_core_states` in the Zig
/// original.
pub const MAX_CORE_STATES: usize = 96;

/// Why a pattern gets no sieve. Every variant is a precondition of soundness or
/// of the cost model — never a taste call — and each is checked against the
/// finished automaton rather than inferred from the pattern's syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decline {
    /// The DFA was built with quit bytes, so it can abandon a search mid-scan. A
    /// continuous run does not model that, and guessing would be unsound.
    Quits,
    /// The start state already matches, so every position accepts and there is
    /// nothing for a filter to reject.
    MatchesEmpty,
    /// The reachable core is wider than `MAX_CORE_STATES` states.
    TooWide,
}

impl core::fmt::Display for Decline {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Quits => write!(
                f,
                "the automaton carries quit bytes and can abandon a scan mid-run"
            ),
            Self::MatchesEmpty => write!(
                f,
                "the start state already matches, so every position accepts"
            ),
            Self::TooWide => write!(f, "the reachable core exceeds {MAX_CORE_STATES} states"),
        }
    }
}

impl core::error::Error for Decline {}

/// A [`Dfa`] projected onto its reachable core: compact state ids, one
/// transition table over derived byte classes, and the accept set.
///
/// `accept` folds the end-of-input transition into the interior one — a state is
/// marked accepting if it matches *or* if it would match at end-of-input. That is
/// deliberately generous: over-marking accepts can only make the sieve pass
/// documents it could have rejected, never reject one that matches, so a `$`- or
/// `\z`-anchored pattern stays sound with no special case.
pub struct Projection {
    /// Number of core states.
    pub states: usize,
    /// Number of byte classes.
    pub classes: usize,
    /// `[states * classes]` compact successor ids.
    pub delta: Vec<u16>,
    /// `[states]` — matches now, or would match at end-of-input.
    pub accept: Vec<bool>,
    /// Compact id of the unanchored start state. Always 0 by construction.
    pub start: u16,
    /// Byte to class.
    pub class_of: [u8; 256],
}

impl Projection {
    /// The successor of core state `s` on class `k`.
    #[inline]
    #[must_use]
    pub fn step(&self, s: u16, k: usize) -> u16 {
        self.delta[usize::from(s) * self.classes + k]
    }

    /// Walk `dfa` and project it, or say why not.
    pub fn of<D: Dfa>(dfa: &D) -> Result<Self, Decline> {
        let start = dfa.start().ok_or(Decline::Quits)?;
        // Mirrors `Row.accepts` below: `regex-automata`'s start state is never itself
        // a match state — even for a pattern that matches empty — because it encodes
        // "about to try", not "already matched". The empty-match acceptance only
        // shows up one step later, at the end-of-input transition.
        if dfa.is_match(start) || dfa.is_match(dfa.next_eoi(start)) {
            return Err(Decline::MatchesEmpty);
        }

        let successors = Self::explore(dfa, start)?;
        let states = successors.len();
        let (class_of, classes) = Self::refine(&successors);

        let mut delta = vec![0u16; states * classes];
        for (s, row) in successors.iter().enumerate() {
            for (byte, &k) in class_of.iter().enumerate() {
                delta[s * classes + usize::from(k)] = row.next[byte];
            }
        }

        Ok(Self {
            states,
            classes,
            delta,
            accept: successors.iter().map(|r| r.accepts).collect(),
            start: 0,
            class_of,
        })
    }

    /// Breadth-first over all 256 bytes, recording every successor so the class
    /// refinement below needs no second walk of the DFA.
    ///
    /// `interned` holds the states seen so far paired with the compact id each was
    /// given, kept sorted by state so a lookup is a binary search. A hash map is the
    /// reflex here and it is the wrong reflex at this size: the core is capped at
    /// [`MAX_CORE_STATES`], so this is at most 96 entries, where seven compares beat
    /// hashing an opaque key — and it asks only [`Ord`] of a caller's state type
    /// rather than [`Hash`](core::hash::Hash) plus a hasher.
    ///
    /// Measured, because "beats hashing" is the kind of claim that is usually wrong:
    /// together with [`Projection::refine`]'s reused column this took the whole
    /// projection from 32 µs to 13 µs on `(?-u)WalletService` and 2.5-3x across the
    /// rest of `examples/bench.rs`'s slate.
    fn explore<D: Dfa>(dfa: &D, start: D::State) -> Result<Vec<Row>, Decline> {
        let mut interned = vec![(start, 0u16)];
        let mut queue = vec![start];
        let mut rows: Vec<Row> = Vec::new();
        let mut head = 0;
        while head < queue.len() {
            let s = queue[head];
            head += 1;
            let mut next = [0u16; 256];
            for (slot, byte) in next.iter_mut().zip(0..=u8::MAX) {
                let t = dfa.next(s, byte);
                if dfa.is_quit(t) {
                    return Err(Decline::Quits);
                }
                *slot = match interned.binary_search_by(|(seen, _)| seen.cmp(&t)) {
                    Ok(at) => interned[at].1,
                    Err(at) => {
                        if queue.len() == MAX_CORE_STATES {
                            return Err(Decline::TooWide);
                        }
                        // `queue.len()` is bounded by MAX_CORE_STATES, so it fits.
                        let fresh = queue.len() as u16;
                        interned.insert(at, (t, fresh));
                        queue.push(t);
                        fresh
                    },
                };
            }
            rows.push(Row {
                next,
                accepts: dfa.is_match(s) || dfa.is_match(dfa.next_eoi(s)),
            });
        }
        Ok(rows)
    }

    /// Group bytes no core state routes differently. This is the exact partition
    /// — the one `regex-automata`'s range-based `ByteClassSet` declines to
    /// compute — and it is what keeps the lattice closure affordable.
    ///
    /// One reused scratch column rather than 256 freshly allocated ones: a class is
    /// identified by the whole successor column, and comparing against the handful
    /// already seen costs less than interning 256 vectors to find that most of them
    /// are duplicates. Real patterns yield well under twenty classes.
    // A partition of 256 bytes has at most 256 blocks, so a block id fits a u8.
    fn refine(rows: &[Row]) -> ([u8; 256], usize) {
        let mut class_of = [0u8; 256];
        let mut seen: Vec<Vec<u16>> = Vec::new();
        let mut column: Vec<u16> = Vec::with_capacity(rows.len());
        for (byte, slot) in class_of.iter_mut().enumerate() {
            column.clear();
            column.extend(rows.iter().map(|r| r.next[byte]));
            *slot = match seen.iter().position(|known| *known == column) {
                Some(known) => known as u8,
                None => {
                    seen.push(column.clone());
                    (seen.len() - 1) as u8
                },
            };
        }
        (class_of, seen.len())
    }
}

/// One core state's successor row, plus whether it accepts. Interned during the
/// breadth-first walk so neither the class refinement nor the table build has to
/// touch the DFA again.
struct Row {
    next: [u16; 256],
    accepts: bool,
}
