//! What does a sieve actually buy? Runs from anywhere; `$SHENG_CORPUS` aims it at the
//! documents you care about instead of the tree this file sits in.
//!
//! Three columns decide it, and none of them is a model:
//!
//! * **armed** — did the lattice yield a register-sized quotient the selectivity
//!   gate accepted, and what fallthrough rate did the gate predict?
//! * **refuted** — the share of genuinely match-free documents the sieve proved
//!   match-free. This is the predicted rate audited against real bytes.
//! * **end to end** — wall time for `regex-automata` alone over every document,
//!   against sieve-then-`regex-automata`-on-survivors. Below 1.00x the sieve is
//!   overhead and should not have armed.

use std::time::Instant;

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::Sieve;

mod common;

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
    r"(?-u)[A-Z][a-z]+Service",
    r"(?-u)#[0-9a-fA-F]{6}",
    r"(?-u)panic!\(",
];

const ROUNDS: usize = 5;

fn matcher(pattern: &str) -> dense::DFA<Vec<u32>> {
    dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .expect("pattern builds")
}

/// What the engine itself says it will do at the start state: the bytes it will
/// `memchr` past, straight from `Automaton::accelerator`. `-` means no skip, so the
/// engine is committed to a per-byte walk — which is the only situation a per-byte
/// sieve can improve on.
fn accel(dfa: &dense::DFA<Vec<u32>>) -> String {
    let Ok(start) = dfa.start_state_forward(&Input::new(b"")) else {
        return "?".to_string();
    };
    match dfa.accelerator(start) {
        [] => "-".to_string(),
        bytes => String::from_utf8_lossy(bytes).escape_debug().to_string(),
    }
}

fn matches(dfa: &dense::DFA<Vec<u32>>, hay: &[u8]) -> bool {
    dfa.try_search_fwd(&Input::new(hay))
        .expect("no quit bytes")
        .is_some()
}

fn main() {
    let docs = common::corpus_files(3000);
    let bytes: usize = docs.iter().map(Vec::len).sum();
    println!(
        "{} documents · {:.1} MiB · min of {ROUNDS} rounds\n",
        docs.len(),
        bytes as f64 / (1 << 20) as f64
    );
    println!(
        "{:<28} {:>9} {:>3} {:>8} {:>10} {:>10} {:>11}",
        "pattern", "armed", "#q", "accel", "engine", "sieved", "end to end"
    );

    // The A/B that justifies the skip kernel existing. `SHENG_NO_SKIP=1` prices and
    // runs every lane on the composition kernel instead, so the two columns this
    // prints can be compared on one machine in one afternoon rather than across a
    // rewrite. It is a knob on the *example*, not on the library: `Policy::skip` is
    // the real seam, and this only spells it.
    let policy = sheng::Policy {
        skip: std::env::var_os("SHENG_NO_SKIP").is_none(),
        ..sheng::Policy::default()
    };
    println!("skip kernel: {}", if policy.skip { "on" } else { "off" });

    let mut armed: Vec<(&str, f64)> = Vec::new();
    for pattern in PATTERNS {
        let dfa = matcher(pattern);
        let sieve = match Sieve::with(pattern, &policy) {
            Ok(s) => s,
            Err(why) => {
                println!(
                    "{pattern:<28} {:>9} {:>3} {:>8}   declined: {why}",
                    "-",
                    "-",
                    accel(&dfa)
                );
                continue;
            },
        };

        let engine = time(ROUNDS, || docs.iter().filter(|d| matches(&dfa, d)).count());
        let sieved = time(ROUNDS, || {
            docs.iter()
                .filter(|d| !sieve.refutes(d) && matches(&dfa, d))
                .count()
        });

        let ratio = engine / sieved;
        armed.push((pattern, ratio));
        println!(
            "{pattern:<28} {:>9.3} {:>3} {:>8} {:>8.2}ms {:>8.2}ms {:>10.3}x",
            sieve.fallthrough(),
            sieve.conjuncts(),
            accel(&dfa),
            engine * 1e3,
            sieved * 1e3,
            ratio
        );
    }

    assert!(
        !armed.is_empty(),
        "no pattern armed — the cost gate has closed entirely"
    );
    let geo = (armed.iter().map(|(_, r)| r.ln()).sum::<f64>() / armed.len() as f64).exp();
    println!(
        "\ngeomean end to end over {} armed patterns: {geo:.3}x",
        armed.len()
    );

    // The gate's whole claim, audited against the clock. Arming is a prediction that
    // this row will come out above 1.000x; a row that armed and lost means a
    // coefficient in `price::ACTIVE` is too generous, not that the row was unlucky.
    let lost: Vec<&(&str, f64)> = armed.iter().filter(|(_, r)| *r < 1.0).collect();
    assert!(
        lost.is_empty(),
        "these rows armed and then lost, so the model that admitted them is wrong: {lost:?}"
    );
    println!("every armed row cleared 1.000x — the gate's predictions held.");
}

fn time(rounds: usize, mut run: impl FnMut() -> usize) -> f64 {
    let mut best = f64::MAX;
    let want = run();
    for _ in 0..rounds {
        let t = Instant::now();
        let got = run();
        best = best.min(t.elapsed().as_secs_f64());
        assert_eq!(got, want, "the two arms disagree — not an optimization");
    }
    best
}
