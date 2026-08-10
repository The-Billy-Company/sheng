//! What a survivor costs, and what the caller would have done instead — the two terms
//! that decide the gate's right-hand side, held to the relationship between them.
//!
//! [`Rival`] alone was once the whole of that side, and it was the wrong whole. A
//! refutation's product is a proof that a document needs **no further work**, so raising
//! the price of that work does raise what a refutation is worth — but only against a
//! caller who would really have paid it on every document. A caller holding a regex
//! would not: the engine decides the same question exactly, for a hundredth of the
//! price, and the sieve is then competing with the engine rather than with the confirm.
//! [`Bypass`] is where that alternative enters the arithmetic, and these tests are the
//! four claims the pair has to keep.
//!
//! 1. **A stated rival is inert while an exact screen exists.** This is the fix. Under
//!    the default baseline, naming a confirm at sixteen thousand walks a byte must reach
//!    exactly the verdict [`Rival::Engine`] reaches, because the engine is what the
//!    caller would run.
//! 2. **Where no exact screen exists, an expensive confirm arms what the engine
//!    declined.** The term is not decorative — it is load-bearing in the one regime it
//!    honestly describes.
//! 3. **Neither term can arm a filter that refutes nothing.** As the confirm's price
//!    grows the gate converges on `survival * (1 + MARGIN) < 1`, so selectivity remains
//!    decisive at every price.
//! 4. **A price that is not a price never arms anything the honest baseline would not.**
//!
//! [`Rival`]: sheng::Rival
//! [`Bypass`]: sheng::Bypass

use sheng::price::Residency;
use sheng::{BuildError, Bypass, Gate, Policy, Rival, Sieve};

/// The regime the shipped rows were minted over, so a decline here is about a price
/// rather than about a column this machine never measured.
const AT: Residency = Residency::Memory;

/// Patterns that harvest a register-sized quotient and then lose to the engine on price
/// — the ordinary, intended outcome, and the population these seams exist to revisit.
///
/// Every one of them is a real filtering surface, and none is chosen for being good at
/// anything: they are here precisely because the gate says no to them today.
const DECLINED_BY_THE_ENGINE: &[&str] = &[
    r"(?-u)#[0-9a-fA-F]{6}",
    r"(?-u)panic!\(",
    r"(?-u)WalletService",
    r"(?-u)unwrap\(\)",
    r"(?-u)[0-9]{3}-[0-9]{4}",
];

/// Filters that pass so many positions that nearly every document survives them. No
/// rival price may rescue these, however large, because the sieve would then be running
/// in front of a confirm it almost never spares.
const LEAKY: &[&str] = &[r"(?-u)a[^\n]*b[^\n]*c", r"(?-u)<[^>]*>"];

/// The slate of rival prices swept, in walks per byte, spanning a plain per-byte
/// checksum up through a decompress-and-parse and on to a network fetch or a model call.
/// Dimensionless on purpose — see [`Rival::Walks`] — so this sweep says the same thing
/// on every machine rather than being a claim about one clock.
const WALKS: &[f64] = &[1.0, 4.0, 16.0, 64.0, 256.0, 1024.0, 16_384.0];

fn at(rival: Rival, bypass: Bypass, gate: Gate) -> Policy<'static> {
    Policy {
        rival,
        bypass,
        gate,
        ..Policy::new(AT)
    }
}

/// **A stated rival is inert while an exact screen exists.**
///
/// The regression guard for the whole objection: before the baseline existed, this
/// population armed at a few hundred walks and the crate advertised that it did. It was
/// arming against a pipeline that puts every document through an OCR pass rather than
/// grepping it first, and every one of those verdicts was two orders of magnitude
/// behind the alternative it was never compared to.
///
/// Asserted as equality of the whole arithmetic rather than of the verdict, because a
/// verdict can agree by luck at one price and the claim is that the confirm's price is
/// not reaching the decision at all.
#[test]
fn an_expensive_confirm_changes_nothing_while_the_engine_can_screen() {
    let mut judged = 0usize;
    for pattern in DECLINED_BY_THE_ENGINE {
        let Ok(engine) = Sieve::with(pattern, &at(Rival::Engine, Bypass::Engines, Gate::Ungated))
        else {
            continue;
        };
        judged += 1;
        for &walks in WALKS {
            let stated = Sieve::with(
                pattern,
                &at(Rival::Walks(walks), Bypass::Engines, Gate::Ungated),
            )
            .expect("the same automaton harvests the same quotient");
            assert_eq!(
                stated.cost().unfiltered(),
                engine.cost().unfiltered(),
                "{pattern:?} at {walks} walks priced its baseline above the engine it \
                 would really have run"
            );
            assert_eq!(
                Sieve::with(
                    pattern,
                    &at(Rival::Walks(walks), Bypass::Engines, Gate::Worth)
                )
                .is_ok(),
                Sieve::with(pattern, &at(Rival::Engine, Bypass::Engines, Gate::Worth)).is_ok(),
                "{pattern:?} at {walks} walks reached a different verdict from the engine \
                 that would have screened it"
            );
        }
    }
    assert!(
        judged > 0,
        "no pattern harvested a quotient, so this test priced nothing"
    );
}

/// **Where no exact screen exists, an expensive confirm arms what the engine declined.**
///
/// [`Bypass::Absent`] is the narrow claim that nothing cheaper can decide the question
/// where the sieve runs — a per-packet refutation against rules whose matches only exist
/// in a reassembled flow. Under it the confirm really is what a survivor costs, and the
/// term has to work.
///
/// Written as a search rather than against a fixed price, because where a given pattern
/// crosses depends on this machine's minted coefficients and pinning it would make this
/// a tripwire for the mint. What is asserted flatly is monotonicity — a caller whose
/// confirm got more expensive can never find the refutation worth *less* — and that some
/// pattern really does cross.
#[test]
fn with_no_screen_available_a_costly_confirm_arms_a_sieve_the_engine_declined() {
    if !sheng::price::active(AT).is_measured(AT) {
        println!("no minted row for this machine — nothing to price a rival against");
        return;
    }

    let (mut crossed, mut priced) = (0usize, 0usize);
    for pattern in DECLINED_BY_THE_ENGINE {
        // The premise. A pattern that already arms against the engine demonstrates
        // nothing here, and one that harvests no quotient cannot be priced at all.
        if Sieve::with(pattern, &Policy::new(AT)).is_ok() {
            continue;
        }
        if Sieve::with(pattern, &at(Rival::Engine, Bypass::Engines, Gate::Ungated)).is_err() {
            continue;
        }
        priced += 1;

        let arms = |walks: f64| {
            Sieve::with(
                pattern,
                &at(Rival::Walks(walks), Bypass::Absent, Gate::Worth),
            )
            .is_ok()
        };
        let Some(first) = WALKS.iter().position(|&walks| arms(walks)) else {
            continue;
        };
        // Monotone in both directions around the crossing: below it the identical sieve
        // declines, at and above it the identical sieve arms.
        for &walks in &WALKS[..first] {
            assert!(
                !arms(walks),
                "{pattern:?} arms at {walks} walks but not at {} — the gate is not \
                 monotone in what a survivor costs",
                WALKS[first]
            );
        }
        for &walks in &WALKS[first..] {
            assert!(
                arms(walks),
                "{pattern:?} declined at {walks} walks after arming at {}",
                WALKS[first]
            );
        }
        crossed += 1;
        println!(
            "{pattern:?} loses to the engine and, with no engine to lose to, arms at {} \
             walks per byte",
            WALKS[first]
        );
    }

    assert!(
        priced > 0,
        "no pattern both harvested a quotient and lost to the engine, so this test \
         priced nothing"
    );
    assert!(
        crossed > 0,
        "no pattern in a population chosen for declining ever armed, at any rival price \
         up to {} walks per byte — Policy::rival is not reaching the gate even with \
         Bypass::Absent",
        WALKS[WALKS.len() - 1]
    );
}

/// **Neither term can arm a filter that refutes nothing.**
///
/// The guardrail that makes the variant above safe to hand a caller. Dividing the
/// pre-pass by an ever more expensive confirm drives the gate toward
/// `survival * (1 + MARGIN) < 1`, so a filter keeping more than `1/(1 + MARGIN)` of all
/// documents is beyond rescue by arithmetic — exactly as it is beyond rescue by slate
/// size. Both terms amortize the pre-pass; neither can manufacture selectivity. Swept
/// under [`Bypass::Absent`], which is the most generous baseline on offer, so a decline
/// here is a decline everywhere.
///
/// The survival figure is checked too, not just the verdict. Asserting only the decline
/// would pass just as well if these patterns had quietly stopped being leaky, which
/// would leave the interesting claim untested.
#[test]
fn no_rival_price_rescues_a_filter_that_retires_nothing() {
    let mut judged = 0usize;
    for pattern in LEAKY {
        let Ok(ungated) = Sieve::with(pattern, &at(Rival::Engine, Bypass::Engines, Gate::Ungated))
        else {
            continue;
        };
        let cost = ungated.cost();
        let survived = sheng::price::survival(cost.fallthrough, cost.len);
        // The premise: these are leaky, or the assertion below is about nothing.
        assert!(
            survived >= 1.0 / (1.0 + sheng::price::MARGIN),
            "{pattern:?} keeps only {:.1}% of documents — it is no longer the leaky \
             case this test was written around",
            survived * 100.0
        );
        // Which is the same statement the ceiling makes, and the one a caller reads:
        // there is no price and no slate that reaches the margin from here.
        assert!(
            cost.ceiling() < 1.0 + sheng::price::MARGIN,
            "{pattern:?} reports a ceiling of {:.3}x, so the decline below is not the \
             ceiling's doing and this test is measuring something else",
            cost.ceiling()
        );
        judged += 1;

        for &walks in WALKS {
            assert!(
                Sieve::with(
                    pattern,
                    &at(Rival::Walks(walks), Bypass::Absent, Gate::Worth)
                )
                .is_err(),
                "{pattern:?} armed at {walks} walks while surviving {:.1}% of documents \
                 — an expensive rival is overriding the gate rather than pricing it",
                survived * 100.0
            );
        }
    }
    assert!(judged > 0, "no leaky pattern harvested a quotient to price");
}

/// A price a caller could not have meant must never arm something the honest baseline
/// would have declined, and must do it by arithmetic rather than by a panic.
///
/// Two readings, because the baseline changed what "declines" means for one of these.
/// With no screen available, none of them may arm: that is the original claim, and it is
/// why `CostFact::pays` guards its comparison — a negative rival does not fail closed on
/// its own, it makes the right-hand side negative and, against a filter leaky enough to
/// survive nearly every document, inverts the inequality and **arms**. With a screen
/// available, an *infinite* confirm is no longer nonsense the gate has to refuse: it is
/// a confirm nobody would reach, and the engine decides. So the claim there is the
/// weaker and more useful one — a nonsense price can only ever cost the caller a sieve
/// the engine would have justified, never buy them one it would not.
#[test]
fn a_nonsense_rival_price_never_arms_more_than_an_honest_one() {
    let nonsense = [
        Rival::Walks(f64::NAN),
        Rival::Walks(-1.0),
        Rival::Walks(0.0),
        Rival::Walks(f64::INFINITY),
        Rival::NanosPerByte(f64::NAN),
        Rival::NanosPerByte(-1.0),
        Rival::NanosPerByte(0.0),
        Rival::NanosPerByte(f64::INFINITY),
    ];
    // A perfectly selective pattern and a leaky one, so the finding does not rest on
    // whichever survival term happens to dominate.
    for pattern in [r"(?-u)(alpha|beta|gamma)", r"(?-u)<[^>]*>"] {
        let honest = Sieve::with(pattern, &at(Rival::Engine, Bypass::Engines, Gate::Worth)).is_ok();
        for rival in nonsense {
            assert!(
                matches!(
                    Sieve::with(pattern, &at(rival, Bypass::Absent, Gate::Worth)),
                    Err(BuildError::NotWorthIt(_) | BuildError::Uncalibrated { .. })
                ),
                "{pattern:?} at {rival:?} with no screen available must decline on the \
                 arithmetic alone"
            );
            assert!(
                Sieve::with(pattern, &at(rival, Bypass::Engines, Gate::Worth)).is_err() || honest,
                "{pattern:?} at {rival:?} armed a sieve the engine's own price declined"
            );
            // …and the refusal stays a policy rather than hardening into an incapacity,
            // exactly as it does for an unmeasured machine.
            assert!(
                Sieve::with(pattern, &at(rival, Bypass::Absent, Gate::Ungated)).is_ok(),
                "{pattern:?} at {rival:?} must still build when the gate is waived"
            );
        }
    }
}
