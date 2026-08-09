//! What the crate does on a machine and a corpus it has never seen.
//!
//! The soundness suite proves a sieve never lies about a document. This one proves the
//! *arming decision* is not quietly a claim about one laptop: an unmeasured machine
//! must decline rather than inherit somebody else's silicon, and a caller who has
//! measured their own must be able to say so and be believed.

use sheng::price::{Calibration, MACOS_AARCH64_NEON, MINTED, Residency, UNMEASURED};
use sheng::prior::{DEFAULT_CHAINS, SOURCE_BYTES};
use sheng::shuffle::Kernel;
use sheng::{BuildError, Gate, Policy, Sieve};

/// The regime every test below prices in. Memory-resident because that is what the
/// shipped rows were minted over, so a decline here is about the policy under test
/// rather than about a regime this machine has no column for.
const AT: Residency = Residency::Memory;

/// Patterns that harvest a register-sized quotient, so a decline below is about the
/// policy rather than about a pattern with no quotient to price.
///
/// The last two arm under the shipped calibration and the first four do not, and both
/// halves are load-bearing: a test that only ever saw declining patterns cannot tell a
/// policy that declines from a policy that is never consulted, which is precisely how
/// `longer_documents_are_harder_to_justify` went vacuous once before.
const ARMING: &[&str] = &[
    r"(?-u)WalletService",
    r"(?-u)foo[^\n]*bar",
    r"(?-u)<[^>]*>",
    r"(?-u)\{[^\n]*\}",
    r"(?-u)(alpha|beta|gamma)",
    r"(?-u)[0-9]{3}-[0-9]{4}",
];

fn with(calibration: Calibration) -> Policy<'static> {
    Policy {
        calibration,
        ..Policy::new(AT)
    }
}

/// The fail-closed core. A machine absent from [`MINTED`] gets no sieve at all — not a
/// sieve priced with another machine's numbers, which is the failure mode that would
/// ship a slowdown to anybody whose silicon we never touched.
#[test]
fn an_unmeasured_machine_declines_instead_of_guessing() {
    let policy = with(UNMEASURED);
    for pattern in ARMING {
        match Sieve::with(pattern, &policy) {
            Err(BuildError::Uncalibrated { os, arch, kernel }) => {
                // Checked against `price::OS` rather than `std::env::consts::OS`, which
                // is not an oracle for this on every target the crate runs on: `std`
                // reports the empty string under `wasi`, where the `cfg`-derived name is
                // `"wasi"` and is the name a row would actually be keyed on. Holding the
                // decline to `std` there asserts a machine must misname itself. Whether
                // the enumerated arms are *spelled* the way `std` spells them is checked
                // where it can be conditioned properly, in `price::calibration`'s
                // `an_enumerated_os_is_spelled_the_way_the_standard_library_spells_it`.
                assert_eq!(
                    os,
                    sheng::price::OS,
                    "decline named the wrong operating system on {}",
                    sheng::price::ARCH
                );
                assert_eq!(
                    arch,
                    std::env::consts::ARCH,
                    "decline named the wrong architecture on {}",
                    sheng::price::OS
                );
                assert_eq!(
                    kernel,
                    sheng::shuffle::kernel(),
                    "decline named the wrong kernel on {}/{}",
                    sheng::price::OS,
                    sheng::price::ARCH
                );
            },
            other => panic!(
                "{pattern:?} must decline on an unmeasured machine ({}/{}), got {other:?}",
                sheng::price::OS,
                sheng::price::ARCH
            ),
        }
    }
}

/// …and the refusal is a policy, not an incapacity: the same pattern on the same
/// unmeasured machine still builds when the caller waives the worth test, so what the
/// gate withholds is the *promise* of a speedup rather than the filter.
#[test]
fn waiving_the_gate_still_builds_on_an_unmeasured_machine() {
    let policy = Policy {
        gate: Gate::Ungated,
        ..with(UNMEASURED)
    };
    for pattern in ARMING {
        let sieve = Sieve::with(pattern, &policy).expect("the quotient does not need a price");
        assert!(sieve.conjuncts() > 0);
        assert!(
            sieve.cost().sieve.is_infinite(),
            "an unmeasured sieve must not report a finite price"
        );
    }
}

/// The seam is load-bearing, not decorative: a caller's own measurement changes the
/// answer. On silicon where `memchr` is no faster than the DFA walk, the engine has no
/// accelerator advantage left and a filter that the shipped calibration declines
/// becomes worth arming.
///
/// `WalletService` is the demonstration again, and the round trip is the point. It
/// stopped moving when the sieve learned to skip its own start block and armed on both
/// machines; it moves once more now that [`sheng::price::MARGIN`] refuses the
/// near-parity edge that arming rested on. Which is the honest shape of the claim: the
/// skip it learned buys nothing against an engine already `memchr`-ing the identical
/// byte, and on silicon where that `memchr` is no faster than a walk the sieve does
/// not need the skip at all — it wins several times over on the composition kernel
/// alone.
///
/// The hex-literal scan it borrowed in the interim is *not* kept as a second case: it
/// prices inside the margin on the handicapped machine, and a test whose demonstration
/// sits a hair from its own threshold is a test that will fail on somebody else's
/// afternoon.
#[test]
fn a_caller_can_price_a_machine_the_crate_never_measured() {
    // A hypothetical target with no fast byte scan: skipping costs what walking costs.
    let flat = Calibration {
        arch: "hypothetical",
        dfa_skip: [1.274907; 2],
        ..MACOS_AARCH64_NEON
    };
    let (mut moved, mut same) = (0, 0);
    let slate = [
        r"(?-u)WalletService",
        r"(?-u)(alpha|beta|gamma)",
        r"(?-u)e[^\n]*q",
    ];
    for pattern in slate {
        let shipped = Sieve::with(pattern, &with(MACOS_AARCH64_NEON));
        let theirs = Sieve::with(pattern, &with(flat));
        if shipped.is_err() && theirs.is_ok() {
            moved += 1;
        }
        if shipped.is_ok() == theirs.is_ok() {
            same += 1;
        }
    }
    assert!(
        moved > 0,
        "no pattern changed verdict between two machines — the calibration is inert"
    );
    // `dfa_skip` now prices both sides — the rival's accelerator and the sieve's own
    // skip loop — so this is no longer the one-sided claim it reads as. It holds
    // because `Lane::plan` takes the cheaper of skip and compose, capping the sieve's
    // exposure at a compose price that does not depend on the coefficient at all,
    // while the rival's exposure has no such ceiling.
    assert!(
        moved + same == slate.len(),
        "a slower memchr must never *un*-arm a pattern"
    );
}

/// A caller's corpus is their fact, so the chains they pass must reach the number the
/// gate decides on — and sweeping more of them must only ever be more pessimistic.
///
/// Both halves matter. Monotonicity is what makes an unknown corpus safe to add a model
/// for; the disagreement check is what proves the chains are read at all rather than
/// being decorative arguments around a hardcoded prior.
#[test]
fn the_prior_reaches_the_decision() {
    let source = [sheng::prior::Prior::Source.chain()];
    let uniform = [sheng::prior::Prior::Uniform.chain()];
    // Ungated: this is a question about the fallthrough model, and it has to be
    // answerable on a machine with no calibration at all.
    let modeled = |pattern: &str, chains: &[sheng::prior::Chain]| {
        Sieve::with(
            pattern,
            &Policy {
                chains,
                gate: Gate::Ungated,
                ..Policy::new(AT)
            },
        )
        .map(|s| s.fallthrough())
    };

    let mut disagreed = 0;
    for pattern in ARMING {
        let (Ok(one), Ok(other), Ok(swept)) = (
            modeled(pattern, &source),
            modeled(pattern, &uniform),
            modeled(pattern, &DEFAULT_CHAINS),
        ) else {
            continue;
        };
        assert!(
            swept >= one.max(other) - 1e-12,
            "{pattern:?}: sweeping chains must dominate each alone \
             ({swept:.3e} vs {one:.3e} / {other:.3e})"
        );
        if (one - other).abs() > 1e-9 {
            disagreed += 1;
        }
    }
    assert!(
        disagreed > 0,
        "no pattern priced differently under source text than under uniform noise — \
         the chains in a Policy are not reaching the model"
    );
}

/// The nominal document length is the caller's fact, and it has to move the answer in
/// the direction the model actually implements: **longer** is the harder sell.
///
/// The survival term is not a fixed cost amortized over the haystack — it is the
/// probability that *at least one* position falls through, `1 - (1-f)^len`, and one
/// survivor drags the engine across the whole document. That probability rises with
/// length, so a long document is likelier to contain a false positive and a filter has
/// to be better to justify itself there. It is the same reasoning that puts
/// `NOMINAL_LEN` at 4 KiB rather than at a line: judging arming over a whole document
/// is deliberate margin against a fallthrough estimate known to be optimistic.
///
/// This test asserted the opposite until the skip kernel landed, and never noticed,
/// because it was **vacuous**: every pattern in `ARMING` declined at the nominal
/// length, so the `continue` below skipped all four and the assertion had never once
/// run. Teaching the sieve to skip its start block armed two of them, the body
/// executed for the first time, and it disagreed with `price::survival` immediately.
/// Hence the census — a test that can quietly stop testing is worse than no test.
#[test]
fn longer_documents_are_harder_to_justify() {
    if !sheng::price::active(AT).is_measured(AT) {
        return; // covered by the unmeasured tests above
    }
    let mut judged = 0;
    for pattern in ARMING {
        // No deal at the nominal length means there is nothing for a longer document
        // to be a worse deal *than*, so those patterns carry no information here.
        let Ok(long) = Sieve::with(pattern, &Policy::new(AT)) else {
            continue;
        };
        // The short build is ungated on purpose: we want its arithmetic even when the
        // gate would (rightly) refuse to hand it back.
        let short = Sieve::with(
            pattern,
            &Policy {
                len: 8.0,
                gate: Gate::Ungated,
                ..Policy::new(AT)
            },
        )
        .expect("ungated always builds");
        let (a, b) = (long.cost().speedup(), short.cost().speedup());
        judged += 1;
        assert!(
            b >= a,
            "{pattern:?}: a 4 KiB document cannot be a better deal than 8 bytes \
             ({a:.3}x vs {b:.3}x) — survival only grows with length"
        );
    }
    assert!(
        judged > 0,
        "no pattern armed at the nominal length, so nothing above was actually \
         compared — this test has gone vacuous again"
    );
}

/// The other end of the same length argument, and the one that is a code path.
///
/// `longer_documents_are_harder_to_justify` covers the direction the model *does*
/// describe: survival grows with length, so a long document is a worse deal. Shortening
/// runs off the end of the measurement instead. Two terms the per-byte model omits — a
/// per-call cost, and the sieve's edge over a walking rival — are each worth less than
/// `MARGIN` down to a few hundred bytes and more than it below, so under the floor the
/// arming decision would rest on exactly the arithmetic that is missing.
///
/// The distinction that matters is which refusal comes back. A sub-floor caller is not
/// being told their pattern does not pay — nothing was priced at all — so `Unmodeled`
/// and `NotWorthIt` must not be interchangeable here, and a pattern that arms at the
/// nominal length is the only way to tell them apart.
#[test]
fn a_document_shorter_than_anything_measured_gets_no_verdict() {
    if !sheng::price::active(AT).is_measured(AT) {
        return; // covered by the unmeasured tests above
    }
    let floor = sheng::price::VALIDITY_FLOOR;
    let at = |len: f64, gate: Gate| Policy {
        len,
        gate,
        ..Policy::new(AT)
    };
    let mut judged = 0;
    for pattern in ARMING {
        // A pattern that already declines at nominal length would decline under the
        // floor too, and could not show *which* refusal the floor is responsible for.
        if Sieve::with(pattern, &Policy::new(AT)).is_err() {
            continue;
        }
        judged += 1;
        for len in [floor - 1.0, 8.0, 0.0, f64::NAN] {
            match Sieve::with(pattern, &at(len, Gate::Worth)) {
                Err(BuildError::Unmodeled {
                    len: got,
                    floor: named,
                }) => {
                    assert!(
                        got.to_bits() == len.to_bits(),
                        "{pattern:?}: decline reported {got} bytes, not the {len} asked for"
                    );
                    assert_eq!(named, floor, "{pattern:?}: decline named the wrong floor");
                },
                other => panic!(
                    "{pattern:?} at {len} bytes must decline as unmodeled rather than \
                     be priced, got {other:?}"
                ),
            }
        }
        // The floor itself is inclusive, and a caller who says they know better still
        // gets a filter — an unmodeled length is a refusal to *promise*, not a refusal
        // to build.
        assert!(
            !matches!(
                Sieve::with(pattern, &at(floor, Gate::Worth)),
                Err(BuildError::Unmodeled { .. })
            ),
            "{pattern:?}: the floor must be a length a verdict can be taken at"
        );
        assert!(
            Sieve::with(pattern, &at(8.0, Gate::Ungated)).is_ok(),
            "{pattern:?}: Gate::Ungated consults no price and must ignore the floor"
        );
    }
    assert!(
        judged > 0,
        "no pattern armed at the nominal length, so no decline above could be \
         attributed to the floor rather than to the price"
    );
}

/// `skip.rs` states the rule in prose — "a rival already `memchr`-ing the identical
/// byte cannot be beaten by a filter that has to find the same byte first" — and this
/// is where it becomes enforced rather than believed.
///
/// It is deliberately **not** a code path. A hard "same set, therefore decline" would
/// be wrong: the two loops search the same needle but excurse into different machines,
/// the engine into a dense DFA whose table misses cache and the sieve into sixteen
/// blocks already in registers, so a genuinely cheaper excursion is a genuinely
/// cheaper sieve and forbidding it would forbid a real win. What the coinciding sets
/// actually do is *cancel the streaming halves of the two prices*, leaving the whole
/// verdict resting on the ratio of two excursion coefficients.
///
/// Which machine you ask therefore decides the answer, and the six-machine mint is what
/// established that. This test used to assert the decline flatly, on one machine where
/// those two coefficients came out 3.5% apart — comfortably inside
/// [`MARGIN`](sheng::price::MARGIN), so the margin dismissed the difference and the
/// pattern declined. Five of the six machines minted since read them 20–30% apart, which
/// the margin cannot dismiss and should not: there the sieve's cheaper restart is a
/// measurement, and `windows`/`aarch64` duly arms.
///
/// So the assertion is the conditional the flat one was a special case of: the pattern
/// declines *unless* this machine's own excursion pair clears the margin, in which case
/// arming is what the numbers say and the adjudicator is `examples/survey.rs`, which
/// times it against real source rather than predicting it. What cannot happen — and is
/// what this still catches — is arming while the two coefficients sit close enough
/// together that only noise separates them.
///
/// All of it is checked. Asserting only the verdict would pass just as well if the
/// pattern declined for some unrelated reason, which would leave the interesting claim
/// — that these are the same search twice — untested.
#[test]
fn a_skip_over_the_engine_s_own_accelerator_cannot_arm_on_the_excursion_ratio() {
    use regex_automata::Input;
    use regex_automata::dfa::{Automaton, dense};
    use regex_automata::nfa::thompson;
    use regex_automata::util::syntax;

    if !sheng::price::active(AT).is_measured(AT) {
        return; // covered by the unmeasured tests above
    }
    let mut checked = 0;
    for pattern in [r"(?-u)WalletService", r"(?-u)foo[^\n]*bar"] {
        let dfa = dense::Builder::new()
            .syntax(syntax::Config::new().utf8(false))
            .thompson(thompson::Config::new().utf8(false))
            .build(pattern)
            .expect("pattern builds");
        let start = dfa
            .start_state_forward(&Input::new(b""))
            .expect("start state");
        let mut accelerator = dfa.accelerator(start).to_vec();
        accelerator.sort_unstable();
        assert!(
            !accelerator.is_empty(),
            "{pattern:?}: the engine must accelerate, or there is no coincidence to test"
        );

        let cal = sheng::price::active(AT);
        let core = sheng::Projection::of(&dfa).expect("projects");
        // The instrument matters, so it is read off the harvested skip rather than assumed:
        // the two restart at genuinely different prices, and taking the cheaper of whatever
        // this pattern actually harvests keeps the bound below one-sided.
        let mut restart = f64::INFINITY;
        for quotient in sheng::harvest(&core) {
            let skip = sheng::Skip::of(&quotient.rows, quotient.start).expect("a skip exists");
            assert_eq!(
                skip.leaves(),
                &accelerator[..],
                "{pattern:?}: this test only says something while the two sets coincide"
            );
            restart = restart.min(cal.skip_excursion[skip.instrument() as usize][AT as usize]);
            checked += 1;
        }

        // Whether this machine measured the sieve's restart cheaper than the engine's by
        // more than the coefficients can be confused by. Exactly the comparison the gate is
        // left holding once the streaming halves cancel.
        let measurably_cheaper =
            cal.dfa_excursion[AT as usize] > restart * (1.0 + sheng::price::MARGIN);
        let cost = Sieve::with(pattern, &Policy::new(AT)).map(|s| s.cost());
        assert!(
            matches!(cost, Err(BuildError::NotWorthIt(_))) || measurably_cheaper,
            "{pattern:?}: searching the engine's own accelerator set armed while its \
             restart ({restart:.3}) is within the margin of the engine's ({:.3}), so only \
             noise separates them, got {cost:?}",
            cal.dfa_excursion[AT as usize]
        );
    }
    assert!(
        checked >= 2,
        "only {checked} quotients — the slate harvested nothing"
    );
}

/// Provenance is not decoration: a row that cannot name its machine cannot be audited,
/// and a row whose kernel is scalar would be pricing the vector economics.
#[test]
fn every_shipped_row_names_a_real_machine() {
    for cal in MINTED {
        assert!(!cal.arch.is_empty() && cal.arch != "unmeasured");
        assert!(
            cal.host.len() > 8,
            "{}: host is not a description",
            cal.arch
        );
        assert_eq!(cal.minted.len(), 10, "{}: minted is not a date", cal.arch);
        assert_ne!(cal.kernel, Kernel::Scalar);
    }
    // And the default prior is a real measurement over real text: a distribution that
    // sums to one, with the byte frequencies of source code rather than of noise.
    let total: f64 = SOURCE_BYTES.iter().sum();
    assert!((total - 1.0).abs() < 1e-6, "byte marginals must sum to 1");
    assert!(
        SOURCE_BYTES[b' ' as usize] > SOURCE_BYTES[b'~' as usize],
        "source text has more spaces than tildes, or this is not source text"
    );
}
