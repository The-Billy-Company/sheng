//! Property tests over the crate's error surface.
//!
//! `tests/soundness.rs` fuzzes the HAYSTACK against a fixed, always-valid pattern
//! slate — every pattern there is built with `.expect("pattern builds")`. That
//! leaves the one input this crate cannot trust — the PATTERN itself — never
//! exercised by anything but a handful of hand-picked examples. A caller's pattern
//! can be malformed, can match the empty string, can blow the lattice past its
//! register cap, or can land on a machine nobody has measured, and every one of
//! those has to come back as a `Result`, never a panic.
//!
//! So where the soundness suite asks "does the sieve ever lie", this file asks the
//! dual question of the build path: for ANY string handed to [`Sieve::new`], does
//! construction always terminate in `Ok` or a well-formed [`BuildError`] — never an
//! unwind — and does every declared failure mode actually explain itself the same
//! way every other one does?

use regex_automata::dfa::dense;
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;
use sheng::price::{CostFact, Residency};
use sheng::{BuildError, Decline, Sieve};

/// xorshift64*, matching `tests/soundness.rs` — a failure is reproducible from its
/// seed alone, with no dependency pulled in just to draw random bytes.
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

/// Pieces a real pattern is built from, so a random assembly of them is regex-shaped
/// far more often than a random byte string is — the same reasoning the soundness
/// fuzz target gives for drawing haystacks around the pattern's own literal bytes,
/// applied here to the pattern instead.
const FRAGMENTS: &[&str] = &[
    "foo",
    "bar",
    "a",
    "WalletService",
    "[a-z]",
    "[^\n]",
    "[0-9]{3}",
    r"\d+",
    r"\w*",
    r"\s?",
    r"\b",
    "(alpha|beta)",
    "(?:x)",
    "x*",
    "y+",
    "z?",
    "^",
    "$",
    r"\z",
    r"\A",
    "{2,5}",
    "(",
    ")",
    "[",
    "]",
    r"\",
    ".",
    "|",
    "(?-u:",
    ")",
    "",
];

fn random_pattern(rng: &mut Rng) -> String {
    let n = 1 + rng.below(6);
    (0..n)
        .map(|_| FRAGMENTS[rng.below(FRAGMENTS.len())])
        .collect()
}

/// Bytes with no regard for UTF-8 or regex syntax at all — decoded lossily, since
/// `Sieve::new` takes `&str` and a real caller's `&str` cannot smuggle invalid
/// UTF-8 in, but replacement characters, raw control bytes and orphaned
/// metacharacters are exactly the shapes a hand-typed pattern arrives in broken.
fn garbage_pattern(rng: &mut Rng) -> String {
    let len = rng.below(24);
    let bytes: Vec<u8> = (0..len).map(|_| (rng.next() & 0xFF) as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A pattern whose reachable core is, by construction, wider than any register-sized
/// closed partition could cover — enough distinct literal branches that determinizing
/// them cannot collapse below the lattice's cap. Deterministic on purpose: `TooWide`
/// must not depend on a fuzz seed to be reachable.
fn too_wide_pattern() -> String {
    let branches: Vec<String> = (0..400).map(|i| format!("lit{i:04}xyz")).collect();
    format!("(?-u)({})", branches.join("|"))
}

/// Runs `f`, converting a panic into a `Result` instead of aborting the sweep — the
/// only way a loop over hundreds of adversarial patterns can say *which one* broke
/// the build path instead of just dying on the first.
fn silently<T>(f: impl FnOnce() -> T + std::panic::UnwindSafe) -> std::thread::Result<T> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(f);
    std::panic::set_hook(previous);
    outcome
}

/// A name for what a build outcome was, coarse enough to census without matching on
/// every field — `NotWorthIt`'s `CostFact` and `Uncalibrated`'s arch string are both
/// machine-dependent, so the census only needs to know which shape of answer came
/// back, not the number inside it.
fn shape(result: &Result<Sieve, BuildError>) -> &'static str {
    match result {
        Ok(_) => "armed",
        Err(BuildError::Automaton(_)) => "automaton",
        Err(BuildError::Shape(Decline::Quits)) => "quits",
        Err(BuildError::Shape(Decline::MatchesEmpty)) => "matches_empty",
        Err(BuildError::Shape(Decline::TooWide)) => "too_wide",
        Err(BuildError::NoQuotient) => "no_quotient",
        Err(BuildError::Uncalibrated { .. }) => "uncalibrated",
        Err(BuildError::Unmodeled { .. }) => "unmodeled",
        Err(BuildError::NotWorthIt(_)) => "not_worth_it",
    }
}

/// The one property that matters for the build path: whatever string a caller
/// spells, `Sieve::new` returns — it never unwinds. 3,000 rounds split between
/// regex-shaped assemblies (most patterns a human actually types) and pure garbage
/// (the ones a fuzzer would find), so both populations get real coverage.
#[test]
fn sieve_construction_never_panics_on_arbitrary_patterns() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let mut seen: std::collections::HashMap<&'static str, usize> = std::collections::HashMap::new();

    for round in 0..3000 {
        let pattern = if round % 2 == 0 {
            random_pattern(&mut rng)
        } else {
            garbage_pattern(&mut rng)
        };
        let outcome = silently(|| Sieve::new(&pattern, Residency::Memory));
        let result = match outcome {
            Ok(result) => result,
            Err(_) => panic!("Sieve::new panicked on pattern {pattern:?} (round {round})"),
        };
        *seen.entry(shape(&result)).or_insert(0) += 1;
        if let Err(err) = &result {
            let text = err.to_string();
            assert!(
                !text.is_empty(),
                "{pattern:?} produced an error with an empty Display: {err:?}"
            );
            assert!(
                text.starts_with("no sieve: "),
                "{pattern:?}: every BuildError explains itself as \"no sieve: …\", got {text:?}"
            );
        }
    }

    eprintln!("sieve_construction_never_panics_on_arbitrary_patterns: {seen:?}");
    assert!(
        seen.get("armed").copied().unwrap_or(0) > 0,
        "no generated pattern armed at all — the generator produces nothing buildable"
    );
    assert!(
        seen.len() > 1,
        "every outcome fell into the same bucket ({seen:?}) — this sweep exercises no diversity"
    );
}

/// `TooWide` and `MatchesEmpty` are reachable through the real pipeline, not just
/// hand-constructed for the Display check below — `Sieve::ungated` is used so the
/// cost gate cannot mask a lattice regression that would otherwise make either
/// pattern merely decline for the wrong reason.
#[test]
fn too_wide_and_matches_empty_are_reachable_through_the_real_pipeline() {
    match Sieve::ungated(&too_wide_pattern()) {
        Err(BuildError::Shape(Decline::TooWide)) => {},
        other => panic!("expected Shape(TooWide), got {other:?}"),
    }
    for pattern in [r"(?-u)a*", r"(?-u).*", r"(?-u)", r"(?-u)a**"] {
        match Sieve::ungated(pattern) {
            Err(BuildError::Shape(Decline::MatchesEmpty)) => {},
            other => panic!("{pattern:?}: expected Shape(MatchesEmpty), got {other:?}"),
        }
    }
}

/// `Quits` cannot happen through `Sieve::new` — this crate's own builder never sets
/// a quit byte — so it is only reachable the way `Sieve::of_dfa` exists for: a
/// caller with their own DFA. Built exactly as `regex-automata`'s own `Config::quit`
/// example does, so the DFA under test is unremarkable except for the one knob this
/// checks.
#[test]
fn quit_bytes_are_reachable_through_a_callers_own_dfa() {
    let dfa = dense::Builder::new()
        .configure(dense::Config::new().quit(b'\n', true))
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(r"(?-u)foo[\x00-\xff]+bar")
        .expect("a quit-byte config still builds a DFA");
    match Sieve::of_dfa(&dfa, Residency::Memory) {
        Err(BuildError::Shape(Decline::Quits)) => {},
        other => panic!("expected Shape(Quits), got {other:?}"),
    }
}

/// Malformed syntax is `BuildError::Automaton`, never any other variant and never a
/// panic — `regex-automata`'s own parser is the oracle for "malformed", so this only
/// asserts the classification, not the parser's own error text.
#[test]
fn malformed_syntax_reports_automaton_not_a_panic() {
    for pattern in [
        "(abc",
        "[a-",
        "a{2,1}",
        r"\p{NotARealClass}",
        "[z-a]",
        "(?P<>x)",
    ] {
        match Sieve::new(pattern, Residency::Memory) {
            Err(BuildError::Automaton(why)) => assert!(
                !why.is_empty(),
                "{pattern:?}: Automaton carried no explanation"
            ),
            other => panic!("{pattern:?}: expected BuildError::Automaton, got {other:?}"),
        }
    }
}

/// Every declared failure mode explains itself the same way every other one does:
/// a non-empty `Display`, no hidden `source()` masquerading as a wrapped error, and —
/// for `BuildError` specifically — the one shared "no sieve: …" prefix every variant
/// carries. This is the regression guard for `Decline` and `BuildError` staying two
/// halves of one idiom instead of drifting back into a Debug-only enum wearing an
/// error's name.
#[test]
fn every_declared_error_variant_explains_itself_consistently() {
    let cost = CostFact {
        fallthrough: 0.5,
        len: 4096.0,
        sieve: 1.0,
        rival: 0.2,
    };
    let build_errors = [
        BuildError::Automaton("unbalanced group".to_string()),
        BuildError::Shape(Decline::Quits),
        BuildError::Shape(Decline::MatchesEmpty),
        BuildError::Shape(Decline::TooWide),
        BuildError::NoQuotient,
        BuildError::Uncalibrated {
            os: "hypothetical",
            arch: "hypothetical",
            kernel: sheng::shuffle::kernel(),
        },
        BuildError::Unmodeled {
            len: 64.0,
            floor: sheng::price::VALIDITY_FLOOR,
        },
        BuildError::NotWorthIt(cost),
    ];
    for err in &build_errors {
        let text = err.to_string();
        assert!(!text.is_empty(), "{err:?} has an empty Display");
        assert!(
            text.starts_with("no sieve: "),
            "{err:?}: expected the shared \"no sieve: …\" prefix, got {text:?}"
        );
        assert!(
            std::error::Error::source(err).is_none(),
            "{err:?} reports a source(), but nothing here wraps another Error"
        );
    }

    for decline in [Decline::Quits, Decline::MatchesEmpty, Decline::TooWide] {
        let text = decline.to_string();
        assert!(!text.is_empty(), "{decline:?} has an empty Display");
        assert!(
            std::error::Error::source(&decline).is_none(),
            "{decline:?} reports a source(), but nothing here wraps another Error"
        );
        // `BuildError::Shape`'s own message must read through this exact text, not a
        // paraphrase of it — the two are meant to be the same information at two
        // tiers, never two descriptions that can drift apart.
        let wrapped = BuildError::Shape(decline).to_string();
        assert!(
            wrapped.contains(&text),
            "BuildError::Shape({decline:?}) = {wrapped:?} does not contain Decline's own \
             Display {text:?} — the two tiers have drifted apart"
        );
    }
}
