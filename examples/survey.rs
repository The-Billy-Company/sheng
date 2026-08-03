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

use std::time::Instant;

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::Sieve;

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

/// The corpus volume below which this refuses to judge the model at all.
///
/// Not a noise threshold — repeating a small measurement makes it precise, not valid.
/// A calibration in `price` is nanoseconds per byte read from memory, and a corpus that
/// fits in cache never reads from memory: at a few hundred kilobytes the engine's own
/// `memchr` accelerator runs at tens of gigabytes a second and beats every per-byte
/// price the crate knows, so a sieve loses on arithmetic that has nothing to do with the
/// sieve. Eight mebibytes is past the last-level cache of the machines this ships for,
/// which is the only property that matters here.
const JUDGEABLE: usize = 8 << 20;

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
    println!(
        "{} documents · {:.1} MiB · {SAMPLES} samples of min-of-{ROUNDS}\n",
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
        ..sheng::Policy::default()
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

    assert!(
        !armed.is_empty(),
        "no pattern armed — the cost gate has closed entirely"
    );
    let geo = (armed.iter().map(|r| r.ratio.ln()).sum::<f64>() / armed.len() as f64).exp();
    println!(
        "\ngeomean end to end over {} armed patterns: {geo:.3}x",
        armed.len()
    );

    // Two things have to hold before a verdict means anything: the corpus has to be in
    // the regime the prices describe, and the row's two arms have to separate. Neither
    // is a softening of the gate — a measurement outside the model's domain is not
    // evidence against the model, and the crate declines an unmeasured machine for the
    // same reason.
    if bytes < JUDGEABLE {
        println!(
            "no verdict: {:.1} MiB is under the {} MiB this needs to judge anything. A \
             corpus this small is cache-resident, so the ratios above price the engine's \
             accelerator against L2 bandwidth rather than the memory the calibration was \
             minted over. Aim $SHENG_CORPUS at a real tree.",
            bytes as f64 / (1 << 20) as f64,
            JUDGEABLE >> 20
        );
        return;
    }

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
