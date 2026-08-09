//! The [`Dfa`] contract, exercised by an automaton this crate did not build.
//!
//! Every other suite here reaches the projection through `regex-automata`, which
//! leaves the claim that makes [`Dfa`] worth having — *any* walkable byte automaton is
//! enough — asserted only by the trait's own signature. A trait with one implementor
//! is not an abstraction, it is a dependency wearing a coat.
//!
//! So the automaton below is written out by hand: four states, three byte classes, no
//! parser, no engine, nothing borrowed. It recognizes **"contains an ASCII digit"**,
//! unanchored, which is a language a test can check against a one-line oracle instead
//! of against a second regex engine.
//!
//! Deliberately shaped so the lattice has something to harvest. A literal chain does
//! not: merging any two of its states cascades into the accepting one and collapses the
//! partition to a single block, which is why `WalletService` needs `regex-automata`'s
//! larger state layout to yield a quotient at all. Here the redundancy is explicit —
//! states `0`/`1` and `2`/`3` are pairwise language-equivalent — so
//! `{{0,1},{2,3}}` is closed under the transition function by construction and the
//! harvest cannot come back empty for a reason that has nothing to do with the trait.

use sheng::price::{MACOS_AARCH64_NEON, Residency, UNMEASURED};
use sheng::{Dfa, Gate, Policy, Sieve};

/// Unanchored "contains an ASCII digit", as a hand-written transition table.
///
/// | state | digit | `a`..=`z` | other | accepts |
/// |---|---|---|---|---|
/// | 0 | 2 | 1 | 0 | no |
/// | 1 | 3 | 1 | 0 | no |
/// | 2 | 2 | 2 | 2 | **yes** |
/// | 3 | 3 | 3 | 3 | **yes** |
///
/// `1` is `0` having just seen a letter and `3` is `2` having got there via one, which
/// changes nothing about the language — that is the point. The two redundant pairs are
/// what give the SP lattice a non-trivial closed partition to find.
struct Digits {
    /// What this automaton claims it will skip past from its start state. A field
    /// rather than a constant so one test can vary it and watch the price move.
    accelerator: &'static [u8],
}

impl Dfa for Digits {
    type State = u8;

    fn start(&self) -> Option<u8> {
        Some(0)
    }

    fn next(&self, state: u8, byte: u8) -> u8 {
        match state {
            0 if byte.is_ascii_digit() => 2,
            1 if byte.is_ascii_digit() => 3,
            0 | 1 if byte.is_ascii_lowercase() => 1,
            0 | 1 => 0,
            // Both accepting states absorb: a digit already seen cannot be unseen.
            absorbing => absorbing,
        }
    }

    /// No `$`-style acceptance in this language, so end of input decides nothing that
    /// the interior transition has not already decided.
    fn next_eoi(&self, state: u8) -> u8 {
        state
    }

    fn is_match(&self, state: u8) -> bool {
        state >= 2
    }

    fn is_quit(&self, _state: u8) -> bool {
        false
    }

    fn accelerator(&self, _state: u8) -> &[u8] {
        self.accelerator
    }
}

impl Digits {
    /// The oracle, which is the language itself rather than a second implementation of
    /// it — there is nothing here for a differential test to be differential against.
    fn matches(hay: &[u8]) -> bool {
        hay.iter().any(u8::is_ascii_digit)
    }
}

/// Haystacks spanning the shapes the kernel treats differently: shorter than a vector
/// step, shorter than a chunk, longer than several chunks, and — the ones that matter —
/// long runs with a single digit planted at every interesting offset.
fn haystacks() -> Vec<Vec<u8>> {
    let mut out = vec![
        Vec::new(),
        b"a".to_vec(),
        b"7".to_vec(),
        b"hello world".to_vec(),
        b"hello w0rld".to_vec(),
        vec![b'z'; 4096],
    ];
    // A filler with no digit in it, so a planted digit is the only match and an
    // off-by-one in the tail cannot coincidentally agree with the oracle.
    for len in [1usize, 15, 16, 17, 63, 255, 256, 257, 1023] {
        out.push(vec![b'q'; len]);
        for at in [0, len / 2, len.saturating_sub(1)] {
            let mut hay = vec![b'q'; len];
            hay[at] = b'5';
            out.push(hay);
        }
    }
    out
}

/// The whole pipeline over an automaton with no `regex-automata` anywhere in it:
/// projection, lattice harvest, selectivity, kernel dispatch.
///
/// Ungated, because the arming gate is a question about *this machine* and this test is
/// a question about the trait — an unmeasured host would otherwise turn a contract
/// check into a skipped one.
#[test]
fn a_hand_written_automaton_drives_the_whole_pipeline() {
    let policy = Policy {
        gate: Gate::Ungated,
        ..Policy::new(Residency::Memory)
    };
    let dfa = Digits {
        accelerator: b"0123456789",
    };
    let sieve = Sieve::of_dfa_with(&dfa, &policy)
        .expect("a hand-written automaton with a closed partition harvests one");
    assert!(
        sieve.conjuncts() > 0,
        "a sieve with no conjuncts would refute nothing and prove nothing here"
    );

    let mut refuted = 0;
    for hay in haystacks() {
        let verdict = sieve.refutes(&hay);
        // The contract, and the only direction it runs: a refutation is a proof, a
        // survival is an opinion.
        assert!(
            !(verdict && Digits::matches(&hay)),
            "refuted a haystack that matches: {:?}",
            String::from_utf8_lossy(&hay)
        );
        // And the vector kernel has to agree with the reference on a caller's
        // automaton exactly as it does on the engine's.
        assert_eq!(
            verdict,
            sieve.refutes_scalar(&hay),
            "kernel and scalar reference disagree on {:?}",
            String::from_utf8_lossy(&hay)
        );
        refuted += usize::from(verdict);
    }
    assert!(
        refuted > 0,
        "nothing was refuted, so the assertion above never had anything to check"
    );
}

/// The projection reads the automaton through the trait rather than around it: an
/// automaton that says it quits must be declined, and one that cannot name a start
/// state must be too.
#[test]
fn the_trait_can_decline_on_the_callers_behalf() {
    /// `Digits`, except every state reports that the search may be abandoned.
    struct Quitting;

    impl Dfa for Quitting {
        type State = u8;

        fn start(&self) -> Option<u8> {
            Some(0)
        }

        fn next(&self, _state: u8, _byte: u8) -> u8 {
            0
        }

        fn next_eoi(&self, state: u8) -> u8 {
            state
        }

        fn is_match(&self, _state: u8) -> bool {
            false
        }

        fn is_quit(&self, _state: u8) -> bool {
            true
        }

        fn accelerator(&self, _state: u8) -> &[u8] {
            &[]
        }
    }

    /// An automaton with no start configuration to offer.
    struct Startless;

    impl Dfa for Startless {
        type State = u8;

        fn start(&self) -> Option<u8> {
            None
        }

        fn next(&self, _state: u8, _byte: u8) -> u8 {
            0
        }

        fn next_eoi(&self, state: u8) -> u8 {
            state
        }

        fn is_match(&self, _state: u8) -> bool {
            false
        }

        fn is_quit(&self, _state: u8) -> bool {
            false
        }

        fn accelerator(&self, _state: u8) -> &[u8] {
            &[]
        }
    }

    let policy = Policy {
        gate: Gate::Ungated,
        ..Policy::new(Residency::Memory)
    };
    for outcome in [
        Sieve::of_dfa_with(&Quitting, &policy),
        Sieve::of_dfa_with(&Startless, &policy),
    ] {
        match outcome {
            Err(sheng::BuildError::Shape(sheng::Decline::Quits)) => {},
            other => panic!("expected Shape(Quits), got {other:?}"),
        }
    }
}

/// [`Dfa::accelerator`] is not decoration — it is the rival's price, and the reason the
/// method is required rather than defaulted. A caller who reports a rare escape byte
/// describes an engine nothing per-byte can beat; one who reports nothing describes an
/// engine committed to walking every byte.
///
/// Priced against a shipped row rather than the running machine's, so the arithmetic
/// under test is the same on every host.
#[test]
fn the_accelerator_a_caller_reports_reaches_the_rivals_price() {
    let policy = Policy {
        calibration: MACOS_AARCH64_NEON,
        gate: Gate::Ungated,
        ..Policy::new(Residency::Memory)
    };
    let priced = |accelerator: &'static [u8]| {
        Sieve::of_dfa_with(&Digits { accelerator }, &policy)
            .expect("ungated always builds")
            .cost()
            .rival
    };
    // 0x07 occurs nowhere in real source; a space is the single commonest byte in it.
    let (rare, common, none) = (priced(b"\x07"), priced(b" "), priced(b""));
    assert!(
        rare < common,
        "a rare escape byte must price a cheaper engine: {rare} vs {common}"
    );
    assert!(
        common <= none,
        "no accelerator can cost more than plain walking: {common} vs {none}"
    );
}

/// The gate applies to a caller's automaton exactly as it does to a pattern: an
/// unmeasured machine declines rather than promising a speedup it cannot price.
#[test]
fn a_callers_automaton_is_gated_like_any_other() {
    let dfa = Digits { accelerator: &[] };
    let unmeasured = Policy {
        calibration: UNMEASURED,
        ..Policy::new(Residency::Memory)
    };
    match Sieve::of_dfa_with(&dfa, &unmeasured) {
        Err(sheng::BuildError::Uncalibrated { os, arch, kernel }) => {
            assert_eq!(os, sheng::price::OS);
            assert_eq!(arch, sheng::price::ARCH);
            assert_eq!(kernel, sheng::shuffle::kernel());
        },
        other => panic!("an unmeasured machine must decline, got {other:?}"),
    }
}
