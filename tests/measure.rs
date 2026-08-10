//! Self-pricing, held to the claim that motivates it: **a machine nobody has minted a
//! row for can price itself and stop being inert.**
//!
//! That is the whole of the reach argument, and it is one assertion —
//! `a_measured_row_prices_a_machine_the_shipped_rows_do_not` builds the same pattern
//! twice, once against [`UNMEASURED`] and once against a row taken here, and the two
//! must differ in *kind*: the first cannot reach a verdict at all, the second reaches one
//! either way. Everything else in this file is about the refusals, which are what keep
//! the claim from being met by returning a plausible-looking row full of noise.
//!
//! The suite runs on a machine that already has a shipped row, so it cannot literally
//! observe `riscv64` coming to life. What it can do is observe the mechanism — an
//! unmeasured calibration replaced by a measured one — which is the same mechanism in
//! both cases and the only part this repository can hold still.
//!
//! [`UNMEASURED`]: sheng::price::UNMEASURED

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::price::{
    self, Bench, Calibration, MEASURABLE_ABOVE, Report, Residency, UNMEASURED, Unmeasurable,
};
use sheng::{BuildError, Policy, Screen, Sieve};

#[path = "../examples/common.rs"]
mod common;

/// Fewer rounds than [`price::ROUNDS`], because none of these assertions is about a
/// coefficient's precision — they are about which coefficients exist and which refusals
/// fire, and a test suite has no business spending seven traversals to learn that.
const ROUNDS: usize = 2;

/// The crate's own tree, which is real source text of a known size.
fn sample() -> Vec<Vec<u8>> {
    common::corpus_bytes(usize::MAX)
}

fn borrowed(docs: &[Vec<u8>]) -> Vec<&[u8]> {
    docs.iter().map(Vec::as_slice).collect()
}

/// A row this machine took for itself, or a panic naming why it could not.
fn mine(docs: &[&[u8]]) -> Calibration {
    Bench::new(docs)
        .rounds(ROUNDS)
        .measure()
        .expect("this machine can price itself over its own source tree")
}

/// The claim. An unmeasured machine cannot reach a verdict; a self-measured one can.
///
/// The two outcomes are deliberately not "declines" versus "arms" — a measured row is
/// still entitled to say `NotWorthIt`, and for most patterns it should. What changes is
/// that the answer is now *arithmetic about this machine* rather than an admission that
/// there is no arithmetic to do, and that difference is the entire reach argument.
#[test]
fn a_measured_row_prices_a_machine_the_shipped_rows_do_not() {
    let docs = sample();
    let docs = borrowed(&docs);
    let taken = mine(&docs);
    let at = taken.regime().expect("a measured row names one regime");

    // A machine with no row: every pattern declines for want of a measurement, whatever
    // its shape and whatever a filter would have been worth.
    let blind = Policy {
        calibration: UNMEASURED,
        ..Policy::new(at)
    };
    let priced = Policy {
        calibration: taken,
        ..Policy::new(at)
    };

    let mut reached = 0usize;
    for pattern in [
        r"(?-u)WalletService",
        r"(?-u)[A-Z][a-z]+Service",
        r"(?-u)(alpha|beta|gamma)",
        r"(?-u)\bTODO\b",
        r"(?-u)AKIA[0-9A-Z]{16}",
    ] {
        let uncalibrated = matches!(
            Sieve::with(pattern, &blind),
            Err(BuildError::Uncalibrated { .. })
        );
        assert!(
            uncalibrated,
            "{pattern:?} reached a verdict with no calibration at all"
        );
        match Sieve::with(pattern, &priced) {
            Err(BuildError::Uncalibrated { .. }) => {
                panic!("{pattern:?} still reads as unpriced under a row measured on this machine")
            },
            // Either verdict is a verdict: what the row bought is the arithmetic, not a
            // guarantee that the arithmetic comes out in the sieve's favour.
            Ok(_) | Err(BuildError::NotWorthIt(_)) => reached += 1,
            // A structural refusal never consulted a price and is unaffected by either
            // policy, which is why it is neither counted nor faulted.
            Err(_) => {},
        }
    }
    assert!(
        reached > 0,
        "no pattern reached a priced verdict — the measured row decided nothing"
    );
}

/// A row taken here has to be a *whole* row for the regime it claims, or
/// [`Calibration::is_measured`] would be reporting a column that cannot price a scan.
#[test]
fn a_measured_row_is_complete_for_the_one_regime_it_measured() {
    let docs = sample();
    let docs = borrowed(&docs);
    let bench = Bench::new(&docs).rounds(ROUNDS);
    let taken = bench.measure().expect("measurable");
    let at = bench.regime();

    assert_eq!(
        taken.regime(),
        Some(at),
        "the row names the sample's regime"
    );
    assert!(taken.is_measured(at));
    assert_eq!(taken.os, price::OS);
    assert_eq!(taken.arch, price::ARCH);
    assert_eq!(taken.kernel, sheng::shuffle::kernel());

    let i = at as usize;
    assert!(taken.dfa_walk > 0.0, "the walk was not timed");
    assert!(taken.dfa_skip[i] > 0.0, "the skip was not timed");
    assert!(
        taken.dfa_excursion[i] >= 1.0,
        "an escape byte cannot cost less than the byte itself, got {}",
        taken.dfa_excursion[i]
    );
    for (slot, instrument) in taken.skip_excursion.iter().enumerate() {
        assert!(
            instrument[i] > 0.0,
            "instrument {slot} priced its excursion at nothing"
        );
    }
    assert!(
        taken.sieve.iter().any(|&ns| ns > 0.0),
        "no conjunct count was timed, so the sieve reads free"
    );

    // And the column it did not measure reads as unmeasured rather than borrowing the
    // one it did — the same refusal a half-minted shipped row makes.
    let other = match at {
        Residency::Cache => Residency::Memory,
        Residency::Memory => Residency::Cache,
    };
    assert!(
        !taken.is_measured(other),
        "one sample is one memory regime, but the row claims {other:?} as well"
    );
    let unpriced = Policy {
        calibration: taken,
        ..Policy::new(other)
    };
    assert!(
        matches!(
            Sieve::with(r"(?-u)WalletService", &unpriced),
            Err(BuildError::Uncalibrated { .. })
        ),
        "the unmeasured column priced a scan anyway"
    );
}

/// The refusals. Each is a case where a row could have been returned and would have been
/// fiction.
#[test]
fn a_sample_that_cannot_be_timed_is_refused_rather_than_guessed() {
    // Too small for the clock: the ratio being measured would be its granularity.
    let tiny = vec![b"some bytes, but nowhere near enough of them".as_slice()];
    assert_eq!(
        Calibration::measure(&tiny).unwrap_err(),
        Unmeasurable::TooFewBytes {
            bytes: tiny[0].len(),
            floor: MEASURABLE_ABOVE
        }
    );
    assert_eq!(
        Calibration::measure(&[]).unwrap_err(),
        Unmeasurable::TooFewBytes {
            bytes: 0,
            floor: MEASURABLE_ABOVE
        }
    );

    // The sentinel, raw. Every reference pattern would match, so every timing would be
    // of an early exit rather than of a traversal.
    let mut binary = vec![b'x'; MEASURABLE_ABOVE * 2];
    binary.splice(4096..4096, *b"\x00\x01zz");
    assert_eq!(
        Calibration::measure(&[&binary]).unwrap_err(),
        Unmeasurable::ProbeMatched
    );

    // Source text that *names* the sentinel in escaped form holds no such bytes, so it
    // is measurable — the check is on the sequence, not on its spelling. This file is
    // such a document, and so is the module the check lives in.
    let escaped = format!(
        "{}{}",
        r"(?-u)\x00\x01zz ",
        "y".repeat(MEASURABLE_ABOVE * 2)
    );
    assert!(Calibration::measure(&[escaped.as_bytes()]).is_ok());
}

/// A sample small enough to be cache-resident says so, and a caller who declares
/// otherwise is refused rather than served the wrong column.
///
/// The inference is the point: everywhere else in this crate a residency is the caller's
/// fact and cannot be probed, but the residency of *a timing loop that is about to run*
/// is decided by the size of the corpus it will sweep, which is in hand.
#[test]
fn the_sample_size_decides_which_column_was_measured() {
    let hot = vec![b'q'; MEASURABLE_ABOVE * 2];
    let docs = [hot.as_slice()];
    let bench = Bench::new(&docs).rounds(1);
    assert_eq!(bench.bytes(), hot.len());
    assert_eq!(
        bench.regime(),
        Residency::Cache,
        "{} bytes is under {} and cannot be a memory-resident measurement",
        hot.len(),
        price::RESIDENT_ABOVE
    );
    assert_eq!(
        bench.measure().expect("measurable").regime(),
        Some(Residency::Cache)
    );
}

/// [`Calibration::merge`] is the only place both columns exist at once, which makes it
/// the only place the cross-regime sanity check can live.
#[test]
fn merging_combines_two_columns_and_refuses_two_machines() {
    let docs = sample();
    let docs = borrowed(&docs);
    let hot = mine(&docs);
    assert_eq!(hot.regime(), Some(Residency::Cache));

    // The other column, forged rather than measured: this test is about `merge`'s rules,
    // and assembling a 64 MiB corpus to exercise them would make it a mint.
    let cold = Calibration {
        dfa_skip: [0.0, hot.dfa_skip[0] * 2.0],
        dfa_excursion: [0.0, hot.dfa_excursion[0]],
        skip_excursion: hot.skip_excursion.map(|per| [0.0, per[0]]),
        ..hot
    };

    let both = hot.merge(&cold).expect("one machine, two columns");
    assert!(both.is_measured(Residency::Cache));
    assert!(both.is_measured(Residency::Memory));
    assert_eq!(both.dfa_skip, [hot.dfa_skip[0], cold.dfa_skip[1]]);
    assert!((both.dfa_walk - hot.dfa_walk).abs() < f64::EPSILON);

    // Order does not matter for the columns themselves.
    let flipped = cold.merge(&hot).expect("one machine, two columns");
    assert_eq!(flipped.dfa_skip, both.dfa_skip);

    // Two machines are two claims, and combining them is exactly the borrowing the
    // whole calibration key exists to prevent.
    let elsewhere = Calibration {
        arch: "riscv64",
        ..cold
    };
    assert!(
        hot.merge(&elsewhere).is_none(),
        "a row from another architecture was absorbed"
    );

    // A cache-resident skip dearer than the memory-resident one is not a finding about a
    // memory system — a hotter haystack cannot cost the engine more — so the pair is
    // refused rather than published.
    let busy = Calibration {
        dfa_skip: [0.0, hot.dfa_skip[0] / 2.0],
        ..cold
    };
    assert!(
        hot.merge(&busy).is_none(),
        "an inverted pair of columns was published as a memory system"
    );
}

/// A row measured on this machine must move what a pattern *costs* and never what it
/// *answers*. Soundness does not come from the calibration, and this is the assertion
/// that says so out loud — a measured row is the one input a caller supplies that the
/// arming gate reads and the refutation kernel does not.
#[test]
fn a_self_measured_row_moves_prices_and_never_answers() {
    let files = common::corpus_paths(200);
    assert!(files.len() >= 16, "expected a real corpus");
    let docs: Vec<&[u8]> = files.iter().map(|(_, bytes)| bytes.as_slice()).collect();
    let taken = mine(&docs);
    let at = taken.regime().expect("one regime");

    for pattern in [
        r"(?-u)WalletService",
        r"(?-u)[A-Z][a-z]+Service",
        r"(?-u)\bTODO\b",
        r"(?-u)panic!\(",
    ] {
        let policy = Policy {
            calibration: taken,
            ..Policy::new(at)
        };
        let screen = Screen::with(pattern, &policy).expect("parses");
        let engine = dense::Builder::new()
            .syntax(syntax::Config::new().utf8(false))
            .thompson(thompson::Config::new().utf8(false))
            .build(pattern)
            .expect("builds");
        for (path, bytes) in &files {
            let want = engine
                .try_search_fwd(&Input::new(bytes.as_slice()))
                .expect("no quit bytes")
                .is_some();
            assert_eq!(
                screen.is_match(bytes),
                want,
                "{pattern:?} on {} disagreed with the engine under a self-measured row",
                path.display()
            );
        }
    }
}

/// The evidence a mint prints has to actually be there, or `examples/mint.rs` publishes
/// a row nobody can judge.
#[test]
fn a_report_carries_the_solutions_the_row_was_built_from() {
    let docs = sample();
    let docs = borrowed(&docs);
    let report = Bench::new(&docs)
        .rounds(ROUNDS)
        .report()
        .expect("measurable");

    assert_eq!(report.bytes, docs.iter().map(|d| d.len()).sum::<usize>());
    assert_eq!(report.at, report.calibration.regime().expect("one regime"));
    assert!(
        !report.engine.is_empty(),
        "no lead byte solved the engine's excursion, so the row took the physical floor"
    );
    let (mean, lo, hi) = Report::spread(&report.engine).expect("solutions exist");
    assert!(lo <= mean && mean <= hi, "the spread is not an interval");
    assert!(
        (report.calibration.dfa_excursion[report.at as usize] - mean).abs() < 1e-9,
        "the row's engine excursion is not the mean of the solutions beside it"
    );
    for (slot, each) in report.probes.iter().enumerate() {
        let Some((.., worst)) = Report::spread(each) else {
            continue;
        };
        assert!(
            (report.calibration.skip_excursion[slot][report.at as usize] - worst).abs() < 1e-9,
            "instrument {slot} did not take the worst of its solutions"
        );
    }
    assert!(
        !report.conjuncts.is_empty(),
        "the slate harvested nothing, so no sieve coefficient was measured"
    );
    assert!(
        Report::spread(&[]).is_none(),
        "an empty slate has no mean to report"
    );
}
