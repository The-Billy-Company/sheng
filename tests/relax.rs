//! The counter-relaxed candidate, held to the two things it claims.
//!
//! Relaxation exists because a bounded repeat costs a DFA state per count, so a whole
//! population of patterns — credentials, fixed-width identifiers, timestamps, every
//! `{n}`-shaped thing anybody greps for — was refused for exceeding the core cap
//! before a single measured coefficient was consulted. Dropping the bound is a
//! **superset** of the pattern's language, which `Dfa`'s contract already permits a
//! sieve to be built from.
//!
//! Two claims, and this file is one test for each:
//!
//! 1. **It is sound.** A superset filter may pass more, and must still never refute a
//!    document the strict pattern matches. Tested the only way that means anything:
//!    with haystacks that really do contain matches, since a random corpus produces a
//!    valid `AKIA[0-9A-Z]{16}` approximately never and a suite that never sees a match
//!    proves nothing about refutation.
//! 2. **It is never a downgrade.** The relaxed automaton is a *candidate*, not a
//!    replacement, so a pattern that already priced well strictly must not come out
//!    worse for the transform existing.
//!
//! Both are checked against `Policy::relax = false`, which is the same build with the
//! transform withheld — so a regression can be attributed to the transform rather than
//! to anything else that moved.

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::price::Residency;
use sheng::{BuildError, Decline, Gate, Policy, Sieve};

/// Counted patterns, each beside a string that genuinely matches it.
///
/// The example is not decoration: it is what makes the soundness test adverse. Every
/// one is asserted to match through `regex-automata` before it is used, so a typo in
/// the second column fails loudly instead of quietly turning the adverse test into a
/// test that only ever sees match-free bytes.
const COUNTED: &[(&str, &str)] = &[
    (r"(?-u)AKIA[0-9A-Z]{16}", "AKIAIOSFODNN7EXAMPLE"),
    (
        r"(?-u)ghp_[0-9A-Za-z]{36}",
        "ghp_0123456789abcdefghijklmnopqrstuvwxyz",
    ),
    (
        r"(?-u)sk_live_[0-9A-Za-z]{24}",
        concat!("sk_", "live_", "4eC39HqLyjWDarjtT1zdp7dc"),
    ),
    (
        r"(?-u)xox[baprs]-[0-9A-Za-z-]{10,48}",
        "xoxb-1234567890abcdef",
    ),
    (r"(?-u)[0-9]{3}-[0-9]{2}-[0-9]{4}", "123-45-6789"),
    (r"(?-u)4[0-9]{12}(?:[0-9]{3})?", "4111111111111"),
    (
        r"(?-u)[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}",
        "192.168.1.1",
    ),
    (
        r"(?-u)[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}",
        "2026-08-09T11:44:00",
    ),
    (
        r"(?-u)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
        "3f2504e0-4f89-11d3-9a0c-0305e82c3301",
    ),
    (r"(?-u)#[0-9a-fA-F]{6}", "#1a2B3c"),
];

fn matcher(pattern: &str) -> dense::DFA<Vec<u32>> {
    dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .expect("pattern builds")
}

fn matches(dfa: &dense::DFA<Vec<u32>>, hay: &[u8]) -> bool {
    dfa.try_search_fwd(&Input::new(hay))
        .expect("no quit bytes")
        .is_some()
}

/// xorshift64*, so a failure is reproducible from its seed alone. Same generator the
/// rest of the suite uses, for the same reason.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// An ungated policy with relaxation either allowed or withheld.
///
/// Ungated throughout, because soundness is a property of the quotient and must hold
/// on every pattern that harvests one — and because the economics are precisely what
/// this file is *not* testing. `Residency::Memory` is arbitrary and decides nothing
/// under `Gate::Ungated`.
fn policy(relax: bool) -> Policy<'static> {
    Policy {
        gate: Gate::Ungated,
        relax,
        ..Policy::new(Residency::Memory)
    }
}

/// The claim that motivated the transform, as a test: these patterns are refused for
/// their *shape* when the bound is kept, and reach the gate when it is dropped.
///
/// Asserted as a majority rather than pattern by pattern, since which side of the
/// core cap a particular `{n}` falls on is a fact about `n` and about
/// `regex-automata`'s determinizer, and pinning each one would make this test a
/// tripwire for their internals. What must not regress is the population: if
/// relaxation stops converting structural refusals into priced candidates, it has
/// stopped doing the one thing it is for.
#[test]
fn dropping_a_bound_converts_a_structural_refusal_into_a_priced_candidate() {
    let (mut strict_refused, mut relaxed_built) = (0usize, 0usize);
    for &(pattern, _) in COUNTED {
        let structural = matches!(
            Sieve::with(pattern, &policy(false)),
            Err(BuildError::Shape(Decline::TooWide) | BuildError::NoQuotient)
        );
        strict_refused += usize::from(structural);
        if structural {
            relaxed_built += usize::from(Sieve::with(pattern, &policy(true)).is_ok());
        }
    }
    assert!(
        strict_refused >= COUNTED.len() / 2,
        "only {strict_refused} of {} counted patterns are refused strictly — this slate no \
         longer exercises the cap it was chosen for",
        COUNTED.len()
    );
    assert!(
        relaxed_built * 2 > strict_refused,
        "relaxation recovered only {relaxed_built} of {strict_refused} structurally refused \
         patterns"
    );
}

/// **Matched implies not refuted**, on haystacks built to contain matches.
///
/// Four haystack shapes per round, because the failure modes differ: the bare example,
/// the example embedded in filler at an arbitrary offset (so the quotient has to dwell
/// before it sees anything), the example with one byte corrupted, and the example
/// truncated. The last two are the near-misses — a relaxed automaton accepts strings
/// the strict pattern rejects, so these are where a filter that had confused the two
/// directions would show it, in either direction: refuting a real match is unsound,
/// and the oracle is asked every time rather than assumed.
#[test]
fn a_relaxed_sieve_never_refutes_a_match() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut confirmed = 0usize;
    let mut harvested = 0usize;

    for &(pattern, example) in COUNTED {
        let dfa = matcher(pattern);
        assert!(
            matches(&dfa, example.as_bytes()),
            "{example:?} does not match {pattern:?} — fix the example, not the test"
        );
        let Ok(sieve) = Sieve::with(pattern, &policy(true)) else {
            continue; // harvested nothing at all: a correct outcome, not a gap
        };
        harvested += 1;

        for round in 0..1500 {
            let hay = match round % 4 {
                0 => example.as_bytes().to_vec(),
                1 => {
                    let mut hay: Vec<u8> = (0..rng.below(200))
                        .map(|_| b"the quick brown fox \n\t{}:-."[rng.below(26)])
                        .collect();
                    let at = rng.below(hay.len() + 1);
                    hay.splice(at..at, example.bytes());
                    hay
                },
                2 => {
                    let mut hay = example.as_bytes().to_vec();
                    let at = rng.below(hay.len());
                    hay[at] = b"0aZ_-. \n"[rng.below(8)];
                    hay
                },
                _ => {
                    let keep = rng.below(example.len() + 1);
                    example.as_bytes()[..keep].to_vec()
                },
            };

            let hit = matches(&dfa, &hay);
            confirmed += usize::from(hit);
            assert!(
                !(hit && sieve.refutes(&hay)),
                "UNSOUND on {}/{}: {pattern:?} matches {:?} but the relaxed sieve refuted it \
                 (round {round})",
                std::env::consts::OS,
                std::env::consts::ARCH,
                String::from_utf8_lossy(&hay)
            );
            // The vector kernel and the reference must also agree on these quotients,
            // which are shapes the rest of the suite's slate does not produce.
            assert_eq!(
                sieve.refutes(&hay),
                sieve.refutes_scalar(&hay),
                "kernel disagreement on {pattern:?} at round {round}"
            );
        }
    }

    assert!(
        harvested * 2 >= COUNTED.len(),
        "only {harvested} of {} counted patterns harvested a quotient",
        COUNTED.len()
    );
    assert!(
        confirmed > 0,
        "no round produced a match — this test proves nothing about refutation"
    );
}

/// Relaxation is an *addition* to the candidate set, so it must never make a decision
/// worse. Both builds are ungated, so what is compared is the arithmetic itself rather
/// than which side of the margin it landed on.
#[test]
fn a_relaxed_candidate_is_never_priced_worse_than_the_strict_one() {
    let mut compared = 0usize;
    for &(pattern, _) in COUNTED
        .iter()
        .chain(&[(r"(?-u)panic!\(", ""), (r"(?-u)\bTODO\b", "")])
    {
        let (Ok(strict), Ok(chosen)) = (
            Sieve::with(pattern, &policy(false)),
            Sieve::with(pattern, &policy(true)),
        ) else {
            continue;
        };
        compared += 1;
        assert!(
            chosen.cost().total() <= strict.cost().total(),
            "{pattern:?}: relaxation was allowed to pick a worse candidate — {:.4} against \
             the strict {:.4} ns/B",
            chosen.cost().total(),
            strict.cost().total()
        );
    }
    assert!(
        compared > 0,
        "no pattern built both ways — this test compared nothing"
    );
}

/// The seam has to be a seam: withholding relaxation must reproduce the strict build
/// exactly, or `Policy::relax` is a knob that cannot be used to measure what the
/// transform costs.
#[test]
fn withholding_relaxation_reproduces_the_strict_build() {
    for pattern in [r"(?-u)[0-9]{3}-[0-9]{2}-[0-9]{4}", r"(?-u)#[0-9a-fA-F]{6}"] {
        let strict = Sieve::with(pattern, &policy(false)).expect("builds strictly");
        let again = Sieve::with(pattern, &policy(false)).expect("builds strictly");
        assert_eq!(
            strict.cost(),
            again.cost(),
            "{pattern:?} is not deterministic without relaxation"
        );
        // And the pattern-string front door with no relaxation must agree with handing
        // the same automaton over directly — the two paths differ only in who parsed.
        let direct = Sieve::of_dfa_with(&matcher(pattern), &policy(false)).expect("builds");
        assert_eq!(
            strict.cost(),
            direct.cost(),
            "{pattern:?}: parsing it here and parsing it there disagree"
        );
    }
}
