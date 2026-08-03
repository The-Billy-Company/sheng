//! The only property that matters: **matched implies not refuted.**
//!
//! A sieve is allowed to pass a document that does not match — that costs a
//! wasted scan and nothing else. It is never allowed to refute a document that
//! does match, because that silently loses a result. So every test here is an
//! adverse one: it hunts for a document the sieve rejects and `regex-automata`
//! matches, and any single instance is a hard failure.
//!
//! The oracle is deliberately `regex-automata`'s own matcher over the same
//! pattern, so a disagreement can only be the sieve's fault. Documents come from
//! three sources with different failure modes: this repository's real source
//! bytes (the distribution nobody chose), mutations that splice pattern
//! fragments into a haystack so multi-byte tails are actually spelled (a random
//! corpus almost never produces a match, and a test that never sees a match
//! proves nothing), and byte strings drawn from a deliberately tiny alphabet
//! where near-misses are dense.

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::Sieve;

// Shares the one corpus walker with the examples instead of keeping a second copy of
// it here — `examples/common.rs` is the version with paths kept beside the bytes,
// which is what a failure message below needs to name the file it came from.
#[path = "../examples/common.rs"]
mod common;

/// Patterns spanning the shapes a sieve is built to see: literal tails, bounded
/// and unbounded dwells, alternations, classes, and repetition.
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

/// Every sieve the lattice can build, whether or not the cost gate would admit it.
///
/// Soundness is a property of the quotient construction, so it has to hold on every
/// pattern that harvests one — not only on the minority the economics admit. Testing
/// the gated path here would silently shrink this suite every time the gate got
/// stricter, which is precisely backwards: the whole point of a refutation filter is
/// that a false reject is a missed match, and that risk exists the moment a quotient
/// exists.
///
/// Patterns that harvest nothing at all are skipped — that is a correct outcome, not
/// a gap — but the suite asserts that most patterns do harvest, so a regression that
/// makes the lattice refuse everything cannot pass by vacuous truth.
fn harvested() -> Vec<(&'static str, Sieve)> {
    let out: Vec<(&'static str, Sieve)> = PATTERNS
        .iter()
        .filter_map(|&p| Sieve::ungated(p).ok().map(|s| (p, s)))
        .collect();
    assert!(
        out.len() * 2 >= PATTERNS.len(),
        "only {} of {} patterns harvested a quotient — the lattice regressed",
        out.len(),
        PATTERNS.len()
    );
    out
}

#[test]
fn a_refutation_is_never_wrong_on_mutated_haystacks() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut confirmed = 0usize;

    for (pattern, sieve) in harvested() {
        let dfa = matcher(pattern);
        // Splice fragments of the pattern's own literal bytes into the haystack,
        // so multi-byte tails get spelled often enough for real matches to occur.
        let alphabet: Vec<u8> = pattern
            .bytes()
            .filter(u8::is_ascii_alphanumeric)
            .chain(b"<>\"{};-# \n".iter().copied())
            .collect();

        for round in 0..4000 {
            let len = 1 + rng.below(96);
            let hay: Vec<u8> = (0..len)
                .map(|_| alphabet[rng.below(alphabet.len())])
                .collect();

            let hit = matches(&dfa, &hay);
            if hit {
                confirmed += 1;
            }
            assert!(
                !(hit && sieve.refutes(&hay)),
                "UNSOUND: {pattern:?} matches {hay:?} but the sieve refuted it (round {round})"
            );
        }
    }

    assert!(
        confirmed > 0,
        "no round produced a match — the mutation strategy proves nothing"
    );
}

#[test]
fn a_refutation_is_never_wrong_on_real_source_bytes() {
    let files = common::corpus_paths(600);
    // A floor, not a skip: too small a corpus makes this test meaningless, so it says
    // so and fails rather than passing over nothing. Sized for a standalone checkout of
    // this crate alone, which is the smallest tree it can legitimately run in.
    assert!(
        files.len() >= 16,
        "expected a real corpus, found {} files under {} — point $SHENG_CORPUS at a source tree",
        files.len(),
        common::root().display()
    );

    let mut matched_any = false;
    for (pattern, sieve) in harvested() {
        let dfa = matcher(pattern);
        for (path, bytes) in &files {
            let hit = matches(&dfa, bytes);
            matched_any |= hit;
            assert!(
                !(hit && sieve.refutes(bytes)),
                "UNSOUND: {pattern:?} matches {} but the sieve refuted it",
                path.display()
            );
        }
    }
    assert!(
        matched_any,
        "no pattern matched any file — the corpus proves nothing"
    );
}

/// Every accelerated kernel must agree byte-for-byte with the scalar reference. A
/// disagreement is a kernel bug that the soundness tests above could mask
/// whenever the scalar path happens to be the one that ran.
///
/// Two census assertions keep this honest, and both exist because the alternative is
/// a green test that compares a path against itself. On a target that has a byte
/// shuffle, dispatch **must** have chosen it. And at least one lane on the slate must
/// have chosen the *skip* kernel, or the unsafe searcher in `skip` is differentiated
/// by nothing at all — a regression that stopped selecting skips would otherwise look
/// exactly like a pass.
///
/// The haystacks matter as much as the count. Uniformly random bytes leave the start
/// block within a byte or two, so a skip loop over them never takes a long jump and
/// never reaches its tail — the one place a vector search goes wrong. So half the
/// rounds draw from a narrow alphabet that keeps a run resident for hundreds of bytes
/// before it escapes.
#[test]
fn every_accelerated_kernel_agrees_with_the_scalar_reference() {
    let kernel = sheng::shuffle::kernel();
    let slate = harvested();
    let skipping: usize = slate.iter().map(|(_, s)| s.skipping()).sum();
    println!(
        "dispatch chose {kernel:?} on {}; {skipping} skip lanes across {} sieves",
        std::env::consts::ARCH,
        slate.len()
    );
    if cfg!(any(target_arch = "aarch64", target_arch = "x86_64")) {
        assert!(
            kernel.is_vector(),
            "{} has a byte shuffle but dispatch chose {kernel:?} — this test would \
             compare the scalar path against itself",
            std::env::consts::ARCH
        );
    }
    if sheng::price::active().is_measured() {
        assert!(
            skipping > 0,
            "no lane on the slate chose the skip kernel, so nothing below differentiates \
             it — either the planner regressed or the slate no longer covers it"
        );
    }

    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for (pattern, sieve) in slate {
        for round in 0..2000 {
            let len = rng.below(1024);
            let hay: Vec<u8> = if round % 2 == 0 {
                (0..len).map(|_| (rng.next() & 0xFF) as u8).collect()
            } else {
                // Long resident runs punctuated by a rare escape, so the skip loop
                // actually jumps and actually lands in its remainder.
                (0..len)
                    .map(|_| {
                        if rng.below(64) == 0 {
                            b"<>{}\";-#\n"[rng.below(9)]
                        } else {
                            b'a' + (rng.below(3)) as u8
                        }
                    })
                    .collect()
            };
            assert_eq!(
                sieve.refutes(&hay),
                sieve.refutes_scalar(&hay),
                "kernel disagreement on {pattern:?} for {len} bytes (round {round})"
            );
        }
    }
}

/// The gate's promise, audited against real bytes.
///
/// Arming is a claim that fronting the engine is cheaper than not, and that claim
/// rests entirely on the sieve retiring most of what it sees. A filter that arms and
/// then refutes a handful of documents is overhead wearing a proof — so this holds
/// every *gated* sieve to a real majority, measured, rather than trusting the model
/// that admitted it.
#[test]
fn an_armed_sieve_retires_most_of_what_it_sees() {
    let files = common::corpus_paths(600);
    let armed: Vec<(&str, Sieve)> = PATTERNS
        .iter()
        .filter_map(|&p| Sieve::new(p).ok().map(|s| (p, s)))
        .collect();
    assert!(
        !armed.is_empty(),
        "no pattern on the slate arms — the cost gate has closed entirely"
    );

    for (pattern, sieve) in armed {
        let dfa = matcher(pattern);
        let clean: Vec<&Vec<u8>> = files
            .iter()
            .map(|(_, b)| b)
            .filter(|b| !matches(&dfa, b))
            .collect();
        if clean.len() < 20 {
            continue; // too few match-free files to say anything
        }
        #[allow(clippy::cast_precision_loss)]
        let share = clean.iter().filter(|b| sieve.refutes(b)).count() as f64 / clean.len() as f64;
        assert!(
            share > 0.5,
            "{pattern:?} armed at {:.3}x on a modeled {:.2e} fallthrough, \
             but retired only {:.1}% of {} match-free files",
            sieve.cost().speedup(),
            sieve.fallthrough(),
            share * 100.0,
            clean.len()
        );
    }
}
