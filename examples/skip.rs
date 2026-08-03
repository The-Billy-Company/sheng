//! Probe: is there a byte-skip hiding in the quotient's start block?
//!
//! The kernel is load-port bound at two loads per byte, so the only remaining
//! speedup is not reading every byte. A quotient's start block usually has a large
//! self-loop set — that is what an unanchored pattern's `.*` prefix looks like after
//! quotienting — and while the run sits in a non-accepting block, consuming
//! self-loop bytes is provably a no-op. Finding the next *escape* byte with a
//! cheaper instrument than the shuffle would skip that run outright.
//!
//! Three numbers decide whether that is worth building, and this reports all three
//! before a line of kernel is written:
//!
//! * **escape width** — how many byte values leave the start block. 1..=3 is
//!   `memchr`; wider needs a byte-set classifier; 256 means there is no skip.
//! * **residency** — the fraction of real corpus bytes the quotient actually spends
//!   in that block, which is the ceiling on what any skip can win.
//! * **the engine's own accelerator** — because a skip that only exists where
//!   `regex-automata` already skips is a skip the cost gate will never arm.

use regex_automata::Input;
use regex_automata::dfa::Automaton;
use regex_automata::dfa::dense;
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::Quotient;

mod common;

const SLATE: &[&str] = &[
    r"(?-u)WalletService",
    r"(?-u)(alpha|beta|gamma)",
    r"(?-u)[A-Z][a-z]+Service",
    r"(?-u)[0-9]{3}-[0-9]{4}",
    r"(?-u)a[^\n]*b",
    r"(?-u)<[^>]*>",
    r"(?-u)#[0-9a-fA-F]{6}",
    r"(?-u)panic!\(",
];

fn main() {
    let docs = common::corpus_bytes(8 << 20);
    println!(
        "{:<26} {:>3} {:>6} {:>7} {:>9} {:>6} {:>9} {:>9} {:>7}",
        "pattern", "#q", "escape", "resident", "skippable", "engine", "compose", "skip", "ratio"
    );

    let (mut audited, mut agreed) = (0usize, 0usize);
    for pattern in SLATE {
        let dfa = dense::Builder::new()
            .syntax(syntax::Config::new().utf8(false))
            .thompson(thompson::Config::new().utf8(false))
            .build(pattern)
            .expect("pattern builds");
        let engine = dfa
            .start_state_forward(&Input::new(b""))
            .ok()
            .map_or(0, |s| dfa.accelerator(s).len());
        let core = sheng::Projection::of(&dfa).expect("projects");
        // What the planner decided, priced from the model alone. The loop below
        // measures what it should have decided; the two are compared at the end.
        let planned = sheng::Sieve::ungated(pattern).map(|s| s.skipping()).ok();
        let mut faster = 0usize;

        for (i, q) in sheng::harvest(&core).iter().enumerate() {
            let escape = escapes(q, q.start);
            let (resident, runs) = residency(q, &docs);
            // What a perfect skip would remove: resident bytes minus one probe per
            // run. A run of one byte is not worth skipping past.
            let skippable = if resident > 0 {
                (resident - runs.min(resident)) as f64 / total(&docs) as f64
            } else {
                0.0
            };
            let compose = per_byte(&docs, |hay| {
                std::hint::black_box(sheng::shuffle::refutes(q, hay));
            });
            let (skip_ns, ratio) = match sheng::Skip::of(&q.rows, q.start) {
                Some(s) if q.start < q.threshold => {
                    let ns = per_byte(&docs, |hay| {
                        std::hint::black_box(sheng::shuffle::refutes_skipping(q, &s, hay));
                    });
                    // Agreement is not optional: a faster wrong answer is the one
                    // failure mode this crate cannot survive.
                    for doc in &docs {
                        assert_eq!(
                            sheng::shuffle::refutes_skipping(q, &s, doc),
                            sheng::shuffle::scalar(q, doc),
                            "skip disagreed with the reference on {pattern}"
                        );
                    }
                    faster += usize::from(ns < compose);
                    (format!("{ns:.4}"), format!("{:.2}x", compose / ns))
                },
                _ => ("-".into(), "-".into()),
            };
            println!(
                "{:<26} {:>3} {:>6} {:>7.1}% {:>8.1}% {:>6} {:>9.4} {:>9} {:>7}",
                if i == 0 { pattern } else { "" },
                i,
                escape.len(),
                100.0 * resident as f64 / total(&docs) as f64,
                100.0 * skippable,
                engine,
                compose,
                skip_ns,
                ratio,
            );
        }
        if let Some(planned) = planned {
            audited += 1;
            agreed += usize::from(planned == faster);
            if planned != faster {
                println!(
                    "{:<26} planner took {planned} skip lane(s), measurement wanted {faster}",
                    ""
                );
            }
        }
    }
    println!(
        "\nplanner agreed with the measurement on {agreed}/{audited} patterns \
         — a disagreement is a mispriced coefficient, not an unsound sieve"
    );
}

/// Byte values that leave `block`. Everything else is a self-loop, and a self-loop
/// on a non-accepting block is a byte the sieve learns nothing from.
fn escapes(q: &Quotient, block: u8) -> Vec<u8> {
    (0..=255u8)
        .filter(|&b| q.rows[usize::from(b)][usize::from(block)] != block)
        .collect()
}

/// Bytes spent in the start block over the real corpus, and how many separate runs
/// they fall into — the run count is what a skip has to pay a probe for.
fn residency(q: &Quotient, docs: &[Vec<u8>]) -> (usize, usize) {
    let (mut resident, mut runs) = (0usize, 0usize);
    for doc in docs {
        let mut state = q.start;
        let mut inside = false;
        for &byte in doc {
            if state == q.start {
                resident += 1;
                if !inside {
                    runs += 1;
                    inside = true;
                }
            } else {
                inside = false;
            }
            state = q.rows[usize::from(byte)][usize::from(state)];
        }
    }
    (resident, runs)
}

fn total(docs: &[Vec<u8>]) -> usize {
    docs.iter().map(Vec::len).sum()
}

/// Nanoseconds per corpus byte, best of a few rounds — this machine routinely has
/// ten coworker agents on it, so the minimum is the least polluted sample.
fn per_byte(docs: &[Vec<u8>], mut run: impl FnMut(&[u8])) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t = std::time::Instant::now();
        for doc in docs {
            run(doc);
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    best * 1e9 / total(docs) as f64
}
