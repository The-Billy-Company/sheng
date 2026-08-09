//! What does a sieve actually buy? Runs from anywhere; `$SHENG_CORPUS` aims it at the
//! documents you care about instead of the tree this file sits in.
//!
//! Three columns decide it, and none of them is a model:
//!
//! * **armed** — did the lattice yield a register-sized quotient the selectivity
//!   gate accepted, and what fallthrough rate did the gate predict?
//! * **refuted** — the share of genuinely match-free documents the sieve proved
//!   match-free. This is the predicted rate audited against real bytes.
//! * **end to end** — wall time for `regex-automata` alone over every document,
//!   against sieve-then-`regex-automata`-on-survivors. Below 1.00x the sieve is
//!   overhead and should not have armed.
//!
//! Every timing is an interval, not a number, and the verdict is taken on the interval.
//! A corpus small enough that the two arms overlap cannot say which is faster, so this
//! declines to judge rather than reporting the clock's own noise as a model error —
//! the same posture the library takes on a machine it has no calibration for.
//!
//! The **regime is read off the corpus**, not configured: this example knows how many
//! bytes it is about to hand the engine, so it declares `Residency::Cache` or
//! `Residency::Memory` to `Policy` accordingly. That is worth knowing about the two
//! numbers this prints, because it used to refuse outright below 8 MiB — a corpus that
//! fits in cache never reads from memory, and a per-byte price measured against memory
//! could not describe it. Running this against a small tree and a large one now
//! exercises two columns of one calibration rather than one column and a disclaimer.
//!
//! `SHENG_SURVEY_REQUIRE_CORPUS=1` turns "declined to judge" into a hard failure — the
//! knob CI's native matrix sets so a corpus checkout that silently shrank cannot pass by
//! having nothing to judge, without forcing that same posture on a human running this
//! against whatever tree they happen to be standing in.

use std::time::Instant;

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::Sieve;
use sheng::price::Residency;

mod common;

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
    r"(?-u)[A-Z][a-z]+Service",
    r"(?-u)#[0-9a-fA-F]{6}",
    r"(?-u)panic!\(",
];

/// Runs per sample. The fastest of a handful is the least-contaminated estimate of a
/// throughput cost, because every source of noise on a shared machine adds time.
const ROUNDS: usize = 5;

/// Samples per arm. One `min`-of-`ROUNDS` is a good estimate with no error bar; several
/// of them disagree by exactly the amount the machine is unable to hold still, which is
/// the quantity a verdict has to clear.
const SAMPLES: usize = 5;

/// The corpus volume above which a scan is reading from memory rather than from cache.
///
/// Eight mebibytes is past the last-level cache of the machines `price::MINTED` names,
/// which is the only property that matters here.
///
/// This constant used to be the volume below which this example **refused to judge the
/// model at all**, and the refusal was correct at the time: a calibration was
/// nanoseconds per byte read from memory, so a corpus that never read from memory was
/// outside the model's domain and a loss there was not evidence against it. That is
/// what `price::Residency` fixed. The threshold now *selects* which column of the
/// calibration the gate reads, so a small corpus is a regime this example can price
/// instead of one it has to decline — which is the entire difference between a model
/// with a documented blind spot and a model with a dimension.
const RESIDENT_ABOVE: usize = 8 << 20;

fn matcher(pattern: &str) -> dense::DFA<Vec<u32>> {
    dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .expect("pattern builds")
}

/// What the engine itself says it will do at the start state: the bytes it will
/// `memchr` past, straight from `Automaton::accelerator`. `-` means no skip, so the
/// engine is committed to a per-byte walk — which is the only situation a per-byte
/// sieve can improve on.
fn accel(dfa: &dense::DFA<Vec<u32>>) -> String {
    let Ok(start) = dfa.start_state_forward(&Input::new(b"")) else {
        return "?".to_string();
    };
    match dfa.accelerator(start) {
        [] => "-".to_string(),
        bytes => String::from_utf8_lossy(bytes).escape_debug().to_string(),
    }
}

fn matches(dfa: &dense::DFA<Vec<u32>>, hay: &[u8]) -> bool {
    dfa.try_search_fwd(&Input::new(hay))
        .expect("no quit bytes")
        .is_some()
}

fn main() {
    let docs = common::corpus_files(3000);
    let bytes: usize = docs.iter().map(Vec::len).sum();
    // The regime is read off the corpus rather than configured, because here it is a
    // measurable fact: this example knows exactly how many bytes it is about to hand the
    // engine and how many times. A library caller has to declare it because the library
    // does not get to see the corpus.
    let residency = if bytes > RESIDENT_ABOVE {
        Residency::Memory
    } else {
        Residency::Cache
    };
    println!(
        "{} documents · {:.1} MiB · {residency:?}-resident · {SAMPLES} samples of min-of-{ROUNDS}\n",
        docs.len(),
        bytes as f64 / (1 << 20) as f64
    );
    println!(
        "{:<28} {:>9} {:>3} {:>8} {:>10} {:>10} {:>11} {:>15}",
        "pattern", "armed", "#q", "accel", "engine", "sieved", "end to end", "interval"
    );

    // The A/B that justifies the skip kernel existing. `SHENG_NO_SKIP=1` prices and
    // runs every lane on the composition kernel instead, so the two columns this
    // prints can be compared on one machine in one afternoon rather than across a
    // rewrite. It is a knob on the *example*, not on the library: `Policy::skip` is
    // the real seam, and this only spells it.
    let policy = sheng::Policy {
        skip: std::env::var_os("SHENG_NO_SKIP").is_none(),
        ..sheng::Policy::new(residency)
    };
    println!("skip kernel: {}", if policy.skip { "on" } else { "off" });

    let mut armed: Vec<Row> = Vec::new();
    for pattern in PATTERNS {
        let dfa = matcher(pattern);
        let sieve = match Sieve::with(pattern, &policy) {
            Ok(s) => s,
            Err(why) => {
                println!(
                    "{pattern:<28} {:>9} {:>3} {:>8}   declined: {why}",
                    "-",
                    "-",
                    accel(&dfa)
                );
                continue;
            },
        };

        let engine = time(|| docs.iter().filter(|d| matches(&dfa, d)).count());
        let sieved = time(|| {
            docs.iter()
                .filter(|d| !sieve.refutes(d) && matches(&dfa, d))
                .count()
        });

        let row = Row {
            pattern,
            engine: engine.best,
            sieved: sieved.best,
            ratio: engine.best / sieved.best,
            // The two extremes the readings admit. `worst / best` is the kindest
            // reading of the sieve, `best / worst` the harshest; a verdict either way
            // has to survive both.
            kindest: engine.worst / sieved.best,
            harshest: engine.best / sieved.worst,
        };
        println!(
            "{pattern:<28} {:>9.3} {:>3} {:>8} {:>8.2}ms {:>8.2}ms {:>10.3}x {:>7.3}-{:.3}x",
            sieve.fallthrough(),
            sieve.conjuncts(),
            accel(&dfa),
            engine.best * 1e3,
            sieved.best * 1e3,
            row.ratio,
            row.harshest,
            row.kindest
        );
        armed.push(row);
    }

    // A corpus in a regime this machine has no column for is the one case still outside
    // the model's domain — and it is now a statement about the *mint* rather than about
    // the corpus. The refusal is kept for exactly that case, because a measurement the
    // calibration cannot price is still not evidence against the calibration. Checked
    // before the census below, since an unpriced regime declines every pattern and
    // "the cost gate has closed entirely" would be a true sentence about the wrong thing.
    if !sheng::price::active(residency).is_measured(residency) {
        let verdict = format!(
            "no verdict: {:.1} MiB is {residency:?}-resident, and no row is minted for \
             this machine in that regime — so every pattern above declined for want of a \
             price rather than for want of a speedup. Mint it with `cargo run --release \
             --example mint`, or aim $SHENG_CORPUS at a corpus in a regime that is.",
            bytes as f64 / (1 << 20) as f64
        );
        // A human running this against whatever tree they happen to be sitting in should
        // get a friendly refusal to judge, not a panic. A CI leg whose pinned corpus
        // checkout silently shrank should not get to call that refusal a pass — this is
        // the one knob that turns "declined to judge" into the failure it would be if it
        // happened on purpose.
        assert!(
            std::env::var_os("SHENG_SURVEY_REQUIRE_CORPUS").is_none(),
            "{verdict}"
        );
        println!("{verdict}");
        return;
    }

    assert!(
        !armed.is_empty(),
        "no pattern armed — the cost gate has closed entirely"
    );
    let geo = (armed.iter().map(|r| r.ratio.ln()).sum::<f64>() / armed.len() as f64).exp();
    println!(
        "\ngeomean end to end over {} armed patterns: {geo:.3}x",
        armed.len()
    );

    // The gate's whole claim, audited against the clock. Arming is a prediction that
    // this row will come out above 1.000x, and a row that loses by more than its own
    // measurement noise means a coefficient in `price::ACTIVE` is too generous.
    let lost: Vec<&Row> = armed.iter().filter(|r| r.kindest < 1.0).collect();
    assert!(
        lost.is_empty(),
        "these rows armed and then lost by more than the clock could account for, so a \
         coefficient in `price::ACTIVE` is too generous:\n{}",
        lost.iter()
            .map(|r| format!(
                "  {:<28} {:.3}x (interval {:.3}-{:.3}x, arms {:.2}ms vs {:.2}ms)",
                r.pattern,
                r.ratio,
                r.harshest,
                r.kindest,
                r.engine * 1e3,
                r.sieved * 1e3
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // A row whose interval straddles 1.000x has not been measured, it has been
    // sampled. Saying so is the whole point: the alternative is a survey that reports
    // the timer's resolution as a verdict, in either direction.
    let unsettled: Vec<&Row> = armed.iter().filter(|r| r.harshest < 1.0).collect();
    if unsettled.is_empty() {
        println!("every armed row cleared 1.000x — the gate's predictions held.");
        return;
    }
    println!(
        "{} of {} armed rows are undecided — their two arms overlap, so this run cannot \
         say which is faster:",
        unsettled.len(),
        armed.len()
    );
    for row in &unsettled {
        println!(
            "  {:<28} {:.3}-{:.3}x over arms of {:.2}ms and {:.2}ms",
            row.pattern,
            row.harshest,
            row.kindest,
            row.engine * 1e3,
            row.sieved * 1e3
        );
    }
    println!(
        "Every other row above is decided. An undecided row is usually one the engine's \
         own accelerator already finishes too fast to time against {:.1} MiB; give it \
         more bytes with $SHENG_CORPUS, or an idler machine, before reading anything \
         into its ratio.",
        bytes as f64 / (1 << 20) as f64
    );
}

/// One armed pattern, and the interval its two timings admit.
#[derive(Debug)]
struct Row {
    pattern: &'static str,
    engine: f64,
    sieved: f64,
    ratio: f64,
    kindest: f64,
    harshest: f64,
}

/// The spread of several independent `min`-of-`ROUNDS` samples for one arm.
///
/// A single minimum is an estimate with no error bar, and a raw maximum is one
/// scheduling hiccup wearing the costume of an error bar. Taking the extremes *of
/// minima* keeps the robustness of the minimum and still says how far the machine
/// moved underneath the measurement.
struct Reading {
    best: f64,
    worst: f64,
}

fn time(mut run: impl FnMut() -> usize) -> Reading {
    let want = run();
    let mut samples = [f64::MAX; SAMPLES];
    for sample in &mut samples {
        for _ in 0..ROUNDS {
            let t = Instant::now();
            let got = run();
            *sample = sample.min(t.elapsed().as_secs_f64());
            assert_eq!(got, want, "the two arms disagree — not an optimization");
        }
    }
    Reading {
        best: samples.iter().copied().fold(f64::MAX, f64::min),
        worst: samples.iter().copied().fold(0.0, f64::max),
    }
}
