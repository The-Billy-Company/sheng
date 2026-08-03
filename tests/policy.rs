//! What the crate does on a machine and a corpus it has never seen.
//!
//! The soundness suite proves a sieve never lies about a document. This one proves the
//! *arming decision* is not quietly a claim about one laptop: an unmeasured machine
//! must decline rather than inherit somebody else's silicon, and a caller who has
//! measured their own must be able to say so and be believed.

use sheng::price::{Calibration, MACOS_AARCH64, MINTED, UNMEASURED};
use sheng::prior::{DEFAULT_CHAINS, SOURCE_BYTES};
use sheng::shuffle::Kernel;
use sheng::{BuildError, Gate, Policy, Sieve};

/// Every pattern the survey arms, so a decline below is about the policy rather than
/// about a pattern with no quotient.
const ARMING: &[&str] = &[
    r"(?-u)WalletService",
    r"(?-u)foo[^\n]*bar",
    r"(?-u)<[^>]*>",
    r"(?-u)\{[^\n]*\}",
];

fn with(calibration: Calibration) -> Policy<'static> {
    Policy {
        calibration,
        ..Policy::default()
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
            Err(BuildError::Uncalibrated { arch, kernel }) => {
                assert_eq!(arch, std::env::consts::ARCH);
                assert_eq!(kernel, sheng::shuffle::kernel());
            },
            other => panic!("{pattern:?} must decline on an unmeasured machine, got {other:?}"),
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
/// `[Ww]allet` used to be the pattern that moved. It now arms on both machines — the
/// sieve learned to skip its own start block, so it no longer needs the engine to be
/// handicapped to be worth it — which left this test asserting a real invariant over a
/// set where nothing moved. A hex-literal scan carries the demonstration instead.
#[test]
fn a_caller_can_price_a_machine_the_crate_never_measured() {
    // A hypothetical target with no fast byte scan: skipping costs what walking costs.
    let flat = Calibration {
        arch: "hypothetical",
        dfa_skip: 1.274907,
        ..MACOS_AARCH64
    };
    let (mut moved, mut same) = (0, 0);
    for pattern in [
        r"(?-u)(alpha|beta|gamma)",
        r"(?-u)#[0-9a-fA-F]{6}",
        r"(?-u)e[^\n]*q",
    ] {
        let shipped = Sieve::with(pattern, &with(MACOS_AARCH64));
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
        moved + same == 3,
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
                ..Policy::default()
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
/// This test asserted the opposite until 2026-08-03, and never noticed, because it was
/// **vacuous**: every pattern in `ARMING` declined at the nominal length, so the
/// `continue` below skipped all four and the assertion had never once run. Teaching
/// the sieve to skip its start block armed two of them, the body executed for the
/// first time, and it disagreed with `price::survival` immediately. Hence the census —
/// a test that can quietly stop testing is worse than no test.
#[test]
fn longer_documents_are_harder_to_justify() {
    if !sheng::price::active().is_measured() {
        return; // covered by the unmeasured tests above
    }
    let mut judged = 0;
    for pattern in ARMING {
        // No deal at the nominal length means there is nothing for a longer document
        // to be a worse deal *than*, so those patterns carry no information here.
        let Ok(long) = Sieve::with(pattern, &Policy::default()) else {
            continue;
        };
        // The short build is ungated on purpose: we want its arithmetic even when the
        // gate would (rightly) refuse to hand it back.
        let short = Sieve::with(
            pattern,
            &Policy {
                len: 8.0,
                gate: Gate::Ungated,
                ..Policy::default()
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
