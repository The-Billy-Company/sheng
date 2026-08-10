//! The front door, held to one promise: **a screen answers exactly what the engine
//! answers.**
//!
//! [`Screen`] exists to absorb a decline rather than report it, which means a caller
//! stops being able to see whether a sieve is in front of the engine or not. That is
//! only an acceptable trade if the two are indistinguishable in their answers, so this
//! file is mostly one differential: for every pattern and every haystack, the screen and
//! `regex-automata` alone must agree. A disagreement in either direction is a bug —
//! `false` where the engine says `true` is a lost match, and `true` where it says `false`
//! is a screen that has stopped consulting its engine.
//!
//! The population is deliberately mixed. Patterns that arm exercise the refute-then-
//! confirm path; patterns that decline exercise the fallback, which is the path most
//! callers will actually be on and which would otherwise be tested by nothing.
//!
//! [`Screen`]: sheng::Screen

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::price::Residency;
use sheng::{BuildError, Screen};

#[path = "../examples/common.rs"]
mod common;

/// Patterns spanning all four census outcomes: shapes that arm, shapes that decline on
/// economics, shapes that were refused structurally before relaxation, and shapes with
/// no counted repeat at all.
const PATTERNS: &[&str] = &[
    r"(?-u)[0-9]{3}-[0-9]{2}-[0-9]{4}",
    r"(?-u)[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}",
    r"(?-u)AKIA[0-9A-Z]{16}",
    r"(?-u)ghp_[0-9A-Za-z]{36}",
    r"(?-u)#[0-9a-fA-F]{6}",
    r"(?-u)\bTODO\b",
    r"(?-u)panic!\(",
    r"(?-u)WalletService",
    r"(?-u)foo[^\n]*bar",
    r"(?-u)https?://[A-Za-z0-9./_-]+",
    r"(?-u)(alpha|beta|gamma)",
    r"(?-u)a*",
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

/// xorshift64*, so a failure is reproducible from its seed alone.
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

/// The promise, over synthetic bytes drawn from the pattern's own alphabet so that real
/// matches actually occur.
#[test]
fn a_screen_answers_exactly_what_the_engine_answers() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF0);
    let mut agreed = 0usize;
    let mut matched = 0usize;

    for &pattern in PATTERNS {
        let screen = Screen::new(pattern, Residency::Memory).expect("the pattern parses");
        let dfa = matcher(pattern);
        let alphabet: Vec<u8> = pattern
            .bytes()
            .filter(u8::is_ascii_alphanumeric)
            .chain(b"-_./:# \n".iter().copied())
            .collect();

        for round in 0..3000 {
            let len = rng.below(160);
            let hay: Vec<u8> = (0..len)
                .map(|_| alphabet[rng.below(alphabet.len())])
                .collect();
            let want = matches(&dfa, &hay);
            matched += usize::from(want);
            agreed += 1;
            assert_eq!(
                screen.is_match(&hay),
                want,
                "{pattern:?} disagreed with the engine on {:?} (round {round}, armed {})",
                String::from_utf8_lossy(&hay),
                screen.armed()
            );
        }
    }

    assert!(agreed > 0, "nothing was compared");
    assert!(
        matched > 0,
        "no haystack matched anything — the differential only exercised the empty answer"
    );
}

/// The same promise over real source text, which is the distribution nobody chose and
/// where the documents are long enough for the survival term to matter.
#[test]
fn a_screen_agrees_with_the_engine_on_real_source_bytes() {
    let files = common::corpus_paths(400);
    assert!(
        files.len() >= 16,
        "expected a real corpus, found {} files under {} — point $SHENG_CORPUS at a source tree",
        files.len(),
        common::root().display()
    );

    for &pattern in PATTERNS {
        let screen = Screen::new(pattern, Residency::Memory).expect("the pattern parses");
        let dfa = matcher(pattern);
        for (path, bytes) in &files {
            assert_eq!(
                screen.is_match(bytes),
                matches(&dfa, bytes),
                "{pattern:?} disagreed with the engine on {} (armed {})",
                path.display(),
                screen.armed()
            );
        }
    }
}

/// A screen never declines, and never pretends the decline did not happen.
///
/// Both halves matter. The first is the type's whole reason to exist: whatever the gate
/// said, a caller gets a working matcher. The second is what keeps that from being a lie
/// by omission — the refusal is still there to read, with its arithmetic, and an
/// unarmed screen says so rather than reporting a filter it does not have.
#[test]
fn a_screen_absorbs_every_decline_without_hiding_it() {
    let mut armed = 0usize;
    let mut declined = 0usize;
    for &pattern in PATTERNS {
        let screen = Screen::new(pattern, Residency::Memory).expect("the pattern parses");
        assert_eq!(
            screen.armed(),
            screen.sieve().is_some(),
            "{pattern:?}: `armed` and `sieve` disagree"
        );
        assert_eq!(
            screen.armed(),
            screen.declined().is_none(),
            "{pattern:?}: a screen is either armed or carries a reason, never both or neither"
        );
        if screen.armed() {
            armed += 1;
            // An armed screen's sieve must be the one the gate admitted, arithmetic
            // included — that is what a caller reads to explain the speedup.
            assert!(screen.sieve().expect("armed").cost().pays());
        } else {
            declined += 1;
            // A screen with no sieve refutes nothing, which is what makes `refutes`
            // safe to branch on without asking `armed` first.
            assert!(!screen.refutes(b"anything at all"));
        }
    }
    assert!(armed > 0, "no pattern on the slate armed");
    assert!(
        declined > 0,
        "every pattern armed — the fallback path this type exists for was never taken"
    );
}

/// The one error a screen may still return, and the one it may not.
///
/// An unparseable pattern is a caller mistake and has to surface. An *economic* refusal
/// must not — if a screen ever returned `NotWorthIt`, it would be the `Sieve` API with
/// extra steps.
#[test]
fn only_an_unparseable_pattern_is_an_error() {
    for pattern in [r"(", r"(?-u)[a-", r"a{2,1}", r"(?P<>x)"] {
        match Screen::new(pattern, Residency::Memory) {
            Err(BuildError::Automaton(_)) => {},
            other => panic!("{pattern:?}: expected an Automaton error, got {other:?}"),
        }
    }
    // A pattern nothing could sieve: the start state already accepts, so every position
    // matches and no filter can reject anything. `Sieve` reports that; `Screen` swallows
    // it and matches anyway.
    let screen = Screen::new(r"(?-u)a*", Residency::Memory).expect("parses");
    assert!(!screen.armed());
    assert!(matches!(
        screen.declined(),
        Some(BuildError::Shape(sheng::Decline::MatchesEmpty))
    ));
    assert!(screen.is_match(b""), "`a*` matches the empty haystack");
}

/// An armed screen has to actually be doing the work, or the differential above would
/// pass just as happily with the sieve wired to `false`.
#[test]
fn an_armed_screen_really_refutes() {
    let files = common::corpus_paths(400);
    let mut audited = 0usize;
    for &pattern in PATTERNS {
        let screen = Screen::new(pattern, Residency::Memory).expect("parses");
        if !screen.armed() {
            continue;
        }
        let dfa = matcher(pattern);
        let clean: Vec<&Vec<u8>> = files
            .iter()
            .map(|(_, bytes)| bytes)
            .filter(|bytes| !matches(&dfa, bytes))
            .collect();
        if clean.len() < 20 {
            continue;
        }
        let retired = clean.iter().filter(|bytes| screen.refutes(bytes)).count();
        audited += 1;
        assert!(
            retired > 0,
            "{pattern:?} armed but refuted none of {} match-free files",
            clean.len()
        );
    }
    assert!(
        audited > 0,
        "no armed pattern had enough match-free files to audit"
    );
}
