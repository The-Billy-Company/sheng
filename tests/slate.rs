//! One sieve in front of a **slate**, and the two walls that decide how much that is
//! ever worth.
//!
//! A caller with fifty rules pays for a pre-pass once and for verification fifty times,
//! which is a different inequality from the one a single pattern faces — see
//! [`Policy::rivals`]. The crate used to describe that as the workload it was best at
//! and never measured it. Measured, the term is real and narrow, and it is bounded twice
//! over:
//!
//! 1. **By what a register can cover.** One quotient has to over-approximate every
//!    member at once. A slate's own union stops being a sieveable object almost
//!    immediately — long before it stops being a slate — so the automaton a fan-out is
//!    declared against is a deliberately coarse superset of one *family* of rules.
//! 2. **By [`CostFact::ceiling`].** The fan-out removes `sieve/rivals` and touches
//!    nothing else, so every slate size converges on `1 / survival`. A filter selective
//!    enough for that to be large is usually already arming at one rival; a filter
//!    marginal enough to need the fan-out is marginal because its survival is high,
//!    which is the one thing the fan-out cannot move.
//!
//! And before either of them matters, the refutation has to be sound across the whole
//! slate, which is the claim a caller cashes when they skip fifty searches.
//!
//! [`Policy::rivals`]: sheng::Policy::rivals
//! [`CostFact::ceiling`]: sheng::price::CostFact::ceiling

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::price::{Residency, survival};
use sheng::{Gate, Policy, Projection, Sieve};

/// A structural slate: no literal prefixes, so no member hands the engine a cheap
/// multi-substring accelerator and the comparison is between a sieve and a walk. Kept
/// short because a union's quotient has to discriminate over every member at once, and
/// how many members fit inside a register is a property of the lattice rather than
/// something a test should pin.
const SLATE: &[&str] = &[
    r"(?-u)[0-9]+-[0-9]+-[0-9]+",
    r"(?-u)[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+",
];

/// Documents that match at least one member, and documents that match none. The second
/// list is what a refutation is supposed to retire; the first is what it must never
/// touch.
const HITS: &[&str] = &[
    "user 123-45-6789 flagged",
    "peer 192.168.10.1 refused",
    "date 2026-08-09 recorded",
];
const CLEAN: &[&str] = &[
    "the quick brown fox jumps over the lazy dog",
    "let mut total = compute(&items).unwrap_or_default();",
    "GET /healthz HTTP/1.1",
];

fn many(patterns: &[&str]) -> dense::DFA<Vec<u32>> {
    dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build_many(patterns)
        .expect("the slate builds")
}

fn one(pattern: &str) -> dense::DFA<Vec<u32>> {
    many(&[pattern])
}

fn matches(dfa: &dense::DFA<Vec<u32>>, hay: &[u8]) -> bool {
    dfa.try_search_fwd(&Input::new(hay))
        .expect("no quit bytes")
        .is_some()
}

/// **A union refutation refutes every member.**
///
/// The union automaton is the filter and one member is the priced rival, which is the
/// arrangement [`Policy::rivals`] documents: the language has to cover the slate, the
/// price has to be one a member really pays. Ungated, because soundness is not a
/// function of the economics.
#[test]
fn a_union_refutation_covers_every_member_of_the_slate() {
    let union = many(SLATE);
    let policy = Policy {
        gate: Gate::Ungated,
        rivals: SLATE.len(),
        ..Policy::new(Residency::Memory)
    };
    let sieve = Sieve::of_superset_with(&union, &one(SLATE[0]), &policy)
        .expect("the slate harvests a quotient");

    let members: Vec<dense::DFA<Vec<u32>>> = SLATE.iter().copied().map(one).collect();
    let mut hit_any = false;
    for text in HITS {
        let hay = text.as_bytes();
        // The premise: each of these really does match somebody, or the assertion below
        // is about nothing.
        let matched: Vec<&str> = SLATE
            .iter()
            .zip(&members)
            .filter(|(_, dfa)| matches(dfa, hay))
            .map(|(pattern, _)| *pattern)
            .collect();
        assert!(
            !matched.is_empty(),
            "{text:?} matches no member of the slate"
        );
        hit_any = true;
        assert!(
            !sieve.refutes(hay),
            "UNSOUND: the union sieve refuted {text:?}, which matches {matched:?}"
        );
        assert_eq!(sieve.refutes(hay), sieve.refutes_scalar(hay));
    }
    assert!(hit_any, "no document exercised a match");

    // And it has to actually retire something, or a "sound" union sieve is just an
    // expensive no-op that would pass the assertion above by refuting nothing at all.
    let retired = CLEAN
        .iter()
        .filter(|text| sieve.refutes(text.as_bytes()))
        .count();
    assert!(
        retired > 0,
        "the union sieve refuted none of {} match-free documents",
        CLEAN.len()
    );
}

/// A wider population of literal-free rules, used only to find where a union stops
/// being sieveable. Every one of them is the kind of rule a secret scanner or an
/// intrusion-detection ruleset is made of, and every one of them harvests a quotient
/// or declines *on its own* — the question here is what happens when they are put in
/// one register together.
const POPULATION: &[&str] = &[
    r"(?-u)[0-9]{3}-[0-9]{2}-[0-9]{4}",
    r"(?-u)[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}",
    r"(?-u)[0-9]{2}/[0-9]{2}/[0-9]{4}",
    r"(?-u)[0-9]+\.[0-9]+\.[0-9]+",
    r"(?-u)[a-z]+_[a-z]+_[a-z]+",
    r"(?-u)[A-F0-9]{2}:[A-F0-9]{2}:[A-F0-9]{2}",
    r"(?-u)[A-Z]{2}[0-9]{2}[A-Z0-9]{4}[0-9]{7}",
    r"(?-u)[0-9a-f]{32}",
];

/// **A slate's union is not a sieveable object, and the wall is very close.**
///
/// This is the measurement the crate was missing while it described a rule slate as its
/// best case. A sixteen-block quotient of a large union — one register retiring a
/// thousand rules per document — is not a filter that got declined on price; it is a
/// filter that does not exist, and the two walls it hits are named here rather than
/// inferred from a decline message.
///
/// Reported as a search and asserted as an inequality, in the direction that catches the
/// claim getting *better*. If the lattice ever harvests over the whole population, this
/// fails and the prose it justifies has to be rewritten — which is the point, for the
/// same reason `only_the_dimensionless_rival_survives_rescaling_the_calibration` asserts
/// its own limitation: a limitation nobody measures is a limitation nobody notices
/// growing.
#[test]
fn a_union_stops_harvesting_long_before_a_slate_stops_growing() {
    let ungated = Policy {
        gate: Gate::Ungated,
        ..Policy::new(Residency::Memory)
    };
    let mut widest = 0usize;
    for size in 1..=POPULATION.len() {
        let union = many(&POPULATION[..size]);
        let core = Projection::of(&union);
        let shape = match &core {
            Ok(p) => format!("{} core states, {} classes", p.states, p.classes),
            Err(why) => format!("{why}"),
        };
        match Sieve::of_superset_with(&union, &union, &ungated) {
            Ok(sieve) => {
                widest = size;
                println!(
                    "{size} rules: {shape} — harvests, passing {:.2e} of positions",
                    sieve.fallthrough()
                );
            },
            Err(why) => println!("{size} rules: {shape} — {why}"),
        }
    }
    println!(
        "the widest union of this population that a register can hold: {widest} of {}",
        POPULATION.len()
    );
    assert!(
        widest < POPULATION.len(),
        "the whole population now fits one quotient — the ceiling this crate documents \
         has moved and the prose about it is stale"
    );
}

/// Slates whose members hand the engine a cheap accelerator, so the sieve and its rival
/// price within a hair of each other and the slate declines at one rival. These are the
/// near-parity cases the fan-out term exists to rescue — a slate that already arms at one
/// rival demonstrates nothing about it.
///
/// The second entry is here for the opposite reason: it is a *leaky* filter, passing
/// enough positions that nearly every document survives, and no slate size may rescue
/// it. Fan-out divides the pre-pass and multiplies both sides of the survival term, so a
/// filter that does not retire documents is exactly as unprofitable across a thousand
/// rules as across one. A term that lifted this over the margin would be arming a sieve
/// that refutes nothing.
const NEAR_PARITY: &[&[&str]] = &[
    &[r"(?-u)0x[0-9a-fA-F]+", r"(?-u)0X[0-9a-fA-F]+"],
    &[r"(?-u)#[0-9a-fA-F]{6}", r"(?-u)#[0-9a-fA-F]{3}"],
];

/// **The fan-out is what admits it, and the ceiling is what limits it.**
///
/// The same sieve, the same machine, the same corpus model — priced once in front of one
/// search and once in front of the slate it really fronts.
///
/// Written as a search rather than a fixed slate size, in both axes, because both are
/// machine-dependent: which slates sit below the margin at one rival depends on this
/// machine's minted coefficients, and so does how many rivals it takes to lift one over.
/// Pinning either would make this a tripwire for the mint. What is asserted without
/// qualification is monotonicity — a caller who adds a rule to a slate can never make
/// the refutation worth less — that *some* slate here really does cross, and that no
/// slate size ever passes the ceiling, which is the bound that makes the first two
/// interesting rather than unlimited.
#[test]
fn a_slate_arms_a_sieve_a_single_pattern_could_not() {
    // A machine with no minted row declines everything for want of a price, which is
    // correct and is not what this test is about.
    if !sheng::price::active(Residency::Memory).is_measured(Residency::Memory) {
        println!("no minted row for this machine — nothing to price a slate against");
        return;
    }

    let mut crossed = 0usize;
    let mut priced = 0usize;
    for slate in NEAR_PARITY {
        let union = many(slate);
        let rival = one(slate[0]);
        let at = |rivals: usize| {
            Sieve::of_superset_with(
                &union,
                &rival,
                &Policy {
                    rivals,
                    ..Policy::new(Residency::Memory)
                },
            )
        };
        // A slate that harvests nothing at all is a correct outcome, not a gap — but it
        // can say nothing here either way.
        let Ok(ungated) = Sieve::of_superset_with(
            &union,
            &rival,
            &Policy {
                gate: Gate::Ungated,
                ..Policy::new(Residency::Memory)
            },
        ) else {
            continue;
        };
        priced += 1;

        // Whatever the slate size, the arithmetic never passes what survival allows.
        let ceiling = ungated.cost().ceiling();
        for rivals in [1usize, 2, 16, 1024, 1 << 20] {
            let speedup = Sieve::of_superset_with(
                &union,
                &rival,
                &Policy {
                    rivals,
                    gate: Gate::Ungated,
                    ..Policy::new(Residency::Memory)
                },
            )
            .expect("the ungated build already succeeded")
            .cost()
            .speedup();
            assert!(
                speedup < ceiling,
                "{slate:?} at {rivals} rivals reached {speedup} past its {ceiling} ceiling"
            );
        }

        let Some(first) = (1..=64).find(|&rivals| at(rivals).is_ok()) else {
            // No slate size arms it, and there is exactly one way that can be correct.
            // As `rivals` grows the pre-pass term vanishes, so the gate converges on
            // `survival * (1 + MARGIN) < 1` — a filter whose survivors keep more than
            // `1/(1 + MARGIN)` of all documents is unrescuable by arithmetic, and a
            // filter with any real selectivity left must therefore have crossed.
            let cost = ungated.cost();
            let survived = survival(cost.fallthrough, cost.len);
            assert!(
                survived >= 1.0 / (1.0 + sheng::price::MARGIN),
                "{slate:?} keeps only {:.1}% of documents yet no slate size arms it — the \
                 fan-out term is not amortizing the pre-pass",
                survived * 100.0
            );
            println!(
                "{slate:?} survives {:.1}% of documents: correctly beyond rescue at any \
                 slate size, ceiling {ceiling:.3}x",
                survived * 100.0
            );
            continue;
        };

        // Monotone: below the crossing the identical sieve declines, at and above it the
        // identical sieve arms.
        for rivals in 1..first {
            assert!(
                at(rivals).is_err(),
                "{slate:?} arms at {rivals} rivals but not at {first} — the gate is not \
                 monotone in the slate size"
            );
        }
        for rivals in first..=first * 2 {
            assert!(
                at(rivals).is_ok(),
                "{slate:?} declined at {rivals} rivals after arming at {first}"
            );
        }
        if first > 1 {
            crossed += 1;
            println!(
                "{slate:?} declines alone and arms at {first} rivals, against a {ceiling:.3}x \
                 ceiling"
            );
        }
    }

    assert!(
        priced > 0,
        "no candidate slate harvested a quotient — this test priced nothing"
    );
    assert!(
        crossed > 0,
        "every candidate slate already armed at one rival, so none of them exercised the \
         fan-out — the term was read but never load-bearing here"
    );
}

/// A coarse superset of a whole *family* of rules — every match that is digits, a
/// separator, digits, a separator, digits. Social security numbers, card numbers,
/// addresses, dates, timestamps and version strings all live inside it, and it is nine
/// states rather than the several hundred their union would need.
///
/// This is the shape a fan-out is honestly declared against, and it is hand-written
/// rather than derived because the derivation does not exist: [`Sieve::of_superset_with`]
/// takes any automaton whose language contains the slate's, and choosing a good one is
/// the caller's modeling problem.
const FAMILY: &str = r"(?-u)[0-9]+[-./:][0-9]+[-./:][0-9]+";

/// **What a slate is worth is decided by record length, not by slate size.**
///
/// The finding that reframes the fan-out. Survival compounds over positions, so the same
/// filter over the same slate is a different proposition at a log line than at a
/// document — and since every amortizing term converges on `1 / survival`, that one
/// input sets the whole ceiling before `rivals` or [`sheng::Rival`] is consulted at all.
///
/// Asserted as a monotone relation rather than at pinned lengths: the ceiling must fall
/// as records grow, and it must fall far enough over the crate's own range that the
/// two ends are different regimes rather than the same one rounded differently.
#[test]
fn the_ceiling_a_slate_can_reach_is_set_by_record_length() {
    let family = dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(FAMILY)
        .expect("the family skeleton builds");

    let ceiling_at = |len: f64| {
        Sieve::of_superset_with(
            &family,
            &family,
            &Policy {
                len,
                gate: Gate::Ungated,
                ..Policy::new(Residency::Memory)
            },
        )
        .expect("the family skeleton harvests")
        .cost()
        .ceiling()
    };

    let lengths = [
        sheng::price::VALIDITY_FLOOR,
        512.0,
        1024.0,
        1500.0,
        sheng::price::NOMINAL_LEN,
        16384.0,
    ];
    let mut previous = f64::INFINITY;
    for len in lengths {
        let ceiling = ceiling_at(len);
        println!("{len:>7.0}-byte records: at most {ceiling:.2}x, ever");
        assert!(
            ceiling <= previous,
            "the ceiling rose from {previous} to {ceiling} at {len} bytes — survival is \
             not compounding over positions"
        );
        previous = ceiling;
    }

    let (short, long) = (
        ceiling_at(sheng::price::VALIDITY_FLOOR),
        ceiling_at(16384.0),
    );
    assert!(
        short > 4.0 * long,
        "a packet ({short:.2}x) and a large document ({long:.2}x) reached the same \
         ceiling — record length is no longer the term that decides a slate"
    );
    assert!(
        long < 1.0 + sheng::price::MARGIN,
        "this family still has {long:.2}x of headroom over 16 KiB records, so it is no \
         longer the case that a slate-wide filter goes leaky on long documents"
    );
}
