//! Every kernel the silicon has against the scalar reference, on the same quotient and
//! the same bytes.
//!
//! `soundness.rs` already differentiates a kernel against `refutes_scalar` — but only
//! the one kernel dispatch chose, and dispatch deliberately chooses the fastest kernel
//! `price::MINTED` has a row for. So the newest instruction set is exactly the one that
//! goes unfuzzed until somebody mints it: `Avx2` is present on essentially every
//! machine that will ever run this fuzzer and unreachable from `soundness.rs` until a
//! row exists. `shuffle::force` is the seam that closes that gap, and it refuses any
//! kernel the runtime probe did not admit, so nothing here can execute an instruction
//! the host lacks.
//!
//! Quotients come from `harvest` directly rather than through a `Sieve`, because the
//! composition kernel's correctness is a property of every quotient the lattice
//! produces, not of the minority the cost gate admits — and a conjunction hides the
//! disagreement of any single member behind the others' refutations.
#![no_main]

use libfuzzer_sys::fuzz_target;
use regex_automata::dfa::dense;
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;
use sheng::{Projection, harvest, shuffle};

/// The slate `soundness.rs` and `tests/soundness.rs` share. Patterns are not fuzzed:
/// a random string is regex noise far more often than it is regex, and the dimension
/// worth exploring here is bytes.
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
    let Some((&index, hay)) = data.split_first() else {
        return;
    };
    let pattern = PATTERNS[usize::from(index) % PATTERNS.len()];

    let dfa = dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .expect("every pattern on the fixed slate builds");
    let Ok(core) = Projection::of(&dfa) else {
        return;
    };

    for quotient in harvest(&core) {
        let want = shuffle::scalar(&quotient, hay);
        for &kernel in shuffle::available() {
            assert!(
                shuffle::force(kernel),
                "{kernel:?} came from available() but force() refused it"
            );
            assert_eq!(
                shuffle::refutes(&quotient, hay),
                want,
                "{kernel:?} disagrees with the scalar reference on {pattern:?} for {hay:?}"
            );
        }
    }
});
