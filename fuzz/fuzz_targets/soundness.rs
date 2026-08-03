//! The same property `tests/soundness.rs` proves over a fixed sweep, handed instead
//! to a coverage-guided fuzzer so byte shapes nobody thought to hand-write get a
//! chance to break it. Patterns are drawn from the crate's own always-valid slate
//! rather than from fuzzer syntax, because a random string is regex noise far more
//! often than it is regex — this way every run spends its whole budget on haystacks,
//! which is the dimension worth exploring.
#![no_main]

use libfuzzer_sys::fuzz_target;
use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;
use sheng::Sieve;

/// Mirrors `tests/soundness.rs::PATTERNS` — the shapes a sieve is built to see:
/// literal tails, bounded and unbounded dwells, alternations, classes, repetition.
const PATTERNS: &[&str] = &[
    r"(?-u)WalletService",
    r"(?-u)foo[^\n]*bar",
    r"(?-u)a[^\n]*b",
    r"(?-u)a[^\n]*b[^\n]*c",
    r"(?-u)<[^>]*>",
    r#"(?-u)"[^"]*;"#,
    r"(?-u)\{[^\n]*\}",
    r"(?-u)(alpha|beta|gamma)",
    r"(?-u)[0-9]{3}-[0-9]{4}",
    r"(?-u)ab+c",
    r"(?-u)x?yz",
    r"(?-u)[A-Z][a-z]+Service",
    r"(?-u)\berror\b",
    r"(?-u)#[0-9a-fA-F]{6}",
    r"(?-u)panic!\(",
];

fuzz_target!(|data: &[u8]| {
    // The first byte selects which pattern this run exercises; everything after it
    // is the haystack, so a corpus entry mutates the two independently.
    let Some((&index, hay)) = data.split_first() else {
        return;
    };
    let pattern = PATTERNS[usize::from(index) % PATTERNS.len()];

    let dfa = dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .expect("every pattern on the fixed slate builds");
    let hit = dfa
        .try_search_fwd(&Input::new(hay))
        .expect("no quit bytes on this slate")
        .is_some();

    // Ungated: soundness is a property of every quotient the lattice harvests, not
    // only of the minority the cost gate happens to admit — see `Sieve::ungated`.
    let Ok(sieve) = Sieve::ungated(pattern) else {
        return; // no quotient for this pattern; nothing to check
    };
    assert!(
        !(hit && sieve.refutes(hay)),
        "UNSOUND: {pattern:?} matches {hay:?} but the sieve refuted it"
    );
    // The vector kernel and the scalar reference must never disagree — a fuzz corpus
    // explores byte shapes the fixed differential sweep in `tests/soundness.rs`
    // would not think to try.
    assert_eq!(
        sieve.refutes(hay),
        sieve.refutes_scalar(hay),
        "kernel disagreement on {pattern:?} for {hay:?}"
    );
});
