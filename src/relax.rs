//! The one transform this crate applies to a pattern before reading its automaton:
//! drop the *upper* bound on every repetition, and pull a lower bound down to one.
//!
//! # Why a pattern shape had to become a build step
//!
//! A bounded repeat costs a DFA state per count. `AKIA[0-9A-Z]{16}` is sixteen
//! counter states, `ghp_[0-9A-Za-z]{36}` is thirty-six, and the reachable core is
//! capped at [`MAX_CORE_STATES`](crate::MAX_CORE_STATES) because the SP-lattice
//! harvest is `O(n²)` closures. So the entire population of credential, identifier
//! and fixed-width record patterns — the shape with the most distinctive alphabet
//! anybody actually greps for — was refused before a single coefficient was
//! consulted. Not declined as unprofitable: never priced. `examples/census.rs`
//! counts it.
//!
//! Relaxing `{16}` to `+` collapses those sixteen states to one. The states were
//! never carrying refutation power in the first place — a quotient of sixteen
//! near-identical counter states is exactly what an SP partition wants to merge —
//! which is why this usually *improves* the harvest rather than trading selectivity
//! for reach.
//!
//! # Why it is sound
//!
//! [`Dfa`](crate::Dfa) already states the obligation: the automaton a sieve is built
//! from must be the confirming one **or one whose language is a superset**. This
//! transform produces a superset, and nothing here relies on a coincidence to do it:
//!
//! * `L(x{n,m}) ⊆ L(x{min(n,1),})` — the lower bound only ever falls and the upper
//!   bound only ever rises, so every string the strict repetition accepts the relaxed
//!   one accepts.
//! * Every `HirKind` combinator is **monotone** in the languages of its
//!   sub-expressions — concatenation, alternation, repetition and capture all
//!   preserve `⊆`, and an `Hir` has no complement or negation operator through which
//!   an enlarged sub-expression could shrink the whole. Look-around in an `Hir` is a
//!   zero-width assertion over surrounding bytes, not a parameterized
//!   sub-expression, so it is a leaf here and cannot invert the direction either.
//!
//! Therefore `L(relaxed) ⊇ L(strict)`, and a refutation by a quotient of the relaxed
//! automaton is a refutation for the strict pattern. The *rival* is still priced from
//! the strict automaton, and the caller still confirms with it — this widens what the
//! filter may pass, never what the search may miss.
//!
//! What it does not do is decide anything. [`crate::Sieve::with`] builds both
//! candidates and keeps whichever the gate prices better, because relaxation can also
//! cost selectivity — `[0-9]{4}[ -]?[0-9]{4}` relaxed passes almost everything — and
//! a transform that is sound in one direction is not thereby profitable in the other.

use alloc::string::ToString;
use alloc::vec::Vec;

use regex_automata::dfa::dense;
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;
use regex_syntax::hir::{Capture, Hir, HirKind, Repetition};

use crate::BuildError;

/// The pattern's own automaton, exactly as written — the one that prices the rival and
/// confirms every survivor.
///
/// Lives beside the relaxation because the two have to agree byte for byte on what
/// alphabet they are over for one to be a superset of the other, and the way to keep
/// two builder configurations in step is to have one of them.
pub(crate) fn strict(pattern: &str) -> Result<dense::DFA<Vec<u32>>, BuildError> {
    dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .map_err(|e| BuildError::Automaton(e.to_string()))
}

/// A superset automaton for `pattern` with every repetition bound dropped, or `None`
/// when there is nothing to gain or nothing to build.
///
/// `None` is the answer in three distinct situations, and none of them is an error a
/// caller should see: the pattern has no bounded repetition, so the relaxed automaton
/// would be the strict one; the pattern does not parse, which the strict build is
/// about to report properly; or the relaxed automaton exceeded a build limit. Every
/// one of them means "carry on with the strict automaton", which is why this returns
/// an option rather than a result — a failure to find a *second* candidate is not a
/// failure to build a sieve.
pub(crate) fn loosened(pattern: &str) -> Option<dense::DFA<Vec<u32>>> {
    // `utf8(false)` on both legs, matching the strict build exactly: a sieve reasons
    // over bytes, and the two automata have to be describing the same alphabet for one
    // to be a superset of the other in any useful sense.
    let hir = syntax::parse_with(pattern, &syntax::Config::new().utf8(false)).ok()?;
    if !bounded(&hir) {
        return None;
    }
    let nfa = thompson::Compiler::new()
        .configure(thompson::Config::new().utf8(false))
        .build_from_hir(&relax(hir))
        .ok()?;
    dense::Builder::new().build_from_nfa(&nfa).ok()
}

/// Does this expression contain a repetition whose bounds [`relax`] would move?
///
/// Checked before relaxing so the common pattern — no counted repeat at all — costs
/// one walk of the tree instead of a rebuild, a Thompson compile and a
/// determinization whose answer is the automaton the caller already has.
fn bounded(hir: &Hir) -> bool {
    let here = match hir.kind() {
        HirKind::Repetition(rep) => rep.min > 1 || rep.max.is_some(),
        _ => false,
    };
    here || hir.kind().subs().iter().any(bounded)
}

/// The transform itself: rebuild `hir` with every repetition unbounded above and
/// bounded by at most one below.
///
/// Exhaustive over `HirKind` rather than falling through a wildcard, because the
/// soundness argument is a claim about *every* combinator — a variant added upstream
/// should stop this compiling and be reasoned about, not silently pass through a
/// catch-all that assumes monotonicity nobody checked.
fn relax(hir: Hir) -> Hir {
    match hir.into_kind() {
        HirKind::Repetition(rep) => Hir::repetition(Repetition {
            // `min(1)` rather than `0`: a repetition that must occur at least once
            // still says so, which keeps a `+` from becoming a `*` and a quotient
            // from having to treat the empty string as a member.
            min: rep.min.min(1),
            max: None,
            greedy: rep.greedy,
            sub: alloc::boxed::Box::new(relax(*rep.sub)),
        }),
        HirKind::Capture(cap) => Hir::capture(Capture {
            index: cap.index,
            name: cap.name,
            sub: alloc::boxed::Box::new(relax(*cap.sub)),
        }),
        HirKind::Concat(subs) => Hir::concat(subs.into_iter().map(relax).collect()),
        HirKind::Alternation(subs) => Hir::alternation(subs.into_iter().map(relax).collect()),
        // Leaves: no sub-expression to relax, and each is reconstructed rather than
        // returned so the match above stays exhaustive.
        HirKind::Empty => Hir::empty(),
        HirKind::Literal(lit) => Hir::literal(lit.0),
        HirKind::Class(class) => Hir::class(class),
        HirKind::Look(look) => Hir::look(look),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The predicate that decides whether a second candidate is even attempted. A
    /// false negative costs the whole point of this module; a false positive costs a
    /// determinization for an automaton identical to one already built.
    #[test]
    fn a_counted_repeat_is_seen_wherever_it_sits() {
        let bounded_of = |pattern: &str| {
            let hir = syntax::parse_with(pattern, &syntax::Config::new().utf8(false)).unwrap();
            bounded(&hir)
        };
        for pattern in [
            r"(?-u)[0-9]{3}",
            r"(?-u)a{2,}",
            r"(?-u)AKIA[0-9A-Z]{16}",
            r"(?-u)(?:ab){2,4}c",
            r"(?-u)x|y{3}",
            r"(?-u)([a-z]{4})",
            r"(?-u)a(b(c{9}))",
        ] {
            assert!(bounded_of(pattern), "{pattern} carries a bound");
        }
        // Stated per pattern rather than derived from the text, which is a trap:
        // every pattern here opens with `(?-u)`, so any rule that reads a `?` out of
        // the spelling classifies the whole slate as bounded.
        for pattern in [
            r"(?-u)[0-9]+",
            r"(?-u)a*b",
            r"(?-u)\bTODO\b",
            r"(?-u)foo[^\n]*bar",
            r"(?-u)(alpha|beta)",
        ] {
            assert!(
                !bounded_of(pattern),
                "{pattern} has no counted repeat and must not be dragged through a rebuild"
            );
        }
        // `a?` is `{0,1}`, so it is a bound like any other and relaxes to `a*`. Called
        // out because it is the one shape where "carries a bound" is not obvious from
        // reading the pattern.
        assert!(bounded_of(r"(?-u)a?"));
    }

    /// Relaxation must be idempotent and must reach every nesting depth: a bound left
    /// behind under a capture or an alternation is a core-state explosion this module
    /// claims to have removed.
    #[test]
    fn relaxing_leaves_no_bound_anywhere() {
        for pattern in [
            r"(?-u)AKIA[0-9A-Z]{16}",
            r"(?-u)([0-9]{3}-){2}[0-9]{4}",
            r"(?-u)(?:a{2}|b{3,7})c{1,9}",
            r"(?-u)\+?[0-9]{1,3}[ .-]?\(?[0-9]{3}\)?[ .-]?[0-9]{3}[ .-]?[0-9]{4}",
        ] {
            let hir = syntax::parse_with(pattern, &syntax::Config::new().utf8(false)).unwrap();
            let once = relax(hir);
            assert!(!bounded(&once), "{pattern} kept a bound through relaxation");
            assert_eq!(
                relax(once.clone()),
                once,
                "{pattern} is not a fixed point of its own relaxation"
            );
        }
    }
}
