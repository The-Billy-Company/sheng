//! Where the time goes, isolated: the kernel's own throughput and the build's own
//! latency, each timed away from the end-to-end slate that `survey` publishes.
//!
//! `survey` answers "is arming worth it", which is the question a caller has. It
//! cannot answer "did the kernel get faster", because a faster kernel changes which
//! rows arm and the geomean moves for two reasons at once. So this measures the two
//! halves directly:
//!
//! * **kernel** — nanoseconds per byte for [`sheng::Sieve::refutes`] over real source,
//!   which is the `sieve` coefficient `mint` writes into a [`sheng::price::Calibration`].
//!   Reported per document size, because the vector path needs a superblock to fill
//!   and a 256-byte document is a different measurement from a 64 KiB one.
//! * **build** — microseconds for [`sheng::Sieve::new`], which a caller compiling one
//!   sieve per literal (regulator does) pays once per literal and notices immediately.
//!
//! Ungated throughout: a gated build on a pattern the economics decline would time
//! nothing at all.

use std::time::Instant;

use regex_automata::dfa::dense;
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::Sieve;

mod common;

/// Patterns that harvest, spanning the shapes the lattice actually sees.
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

const ROUNDS: usize = 7;

fn main() {
    let docs = common::corpus_bytes(32 << 20);
    let bytes: usize = docs.iter().map(Vec::len).sum();
    println!(
        "{} · {} documents · {:.1} MiB\n",
        common::host(),
        docs.len(),
        bytes as f64 / (1 << 20) as f64
    );

    println!(
        "{:<28} {:>3} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "pattern", "#q", "project µs", "harvest µs", "select µs", "build µs", "kernel ns/B"
    );
    let mut kernel_ns = Vec::new();
    for pattern in SLATE {
        let Ok(sieve) = Sieve::ungated(pattern) else {
            println!("{pattern:<28} {:>3}   no quotient", "-");
            continue;
        };
        let (project, harvest, select) = phases(pattern);
        let build = fastest(ROUNDS, || {
            std::hint::black_box(Sieve::ungated(pattern).is_ok());
        });
        let ns = per_byte(&docs, |hay| {
            std::hint::black_box(sieve.refutes(hay));
        });
        kernel_ns.push(ns);
        println!(
            "{pattern:<28} {:>3} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>12.4}",
            sieve.conjuncts(),
            project * 1e6,
            harvest * 1e6,
            select * 1e6,
            build * 1e6,
            ns
        );
    }

    // The geomean is the headline the calibration's `sieve` coefficient tracks; the
    // spread beside it is how much a single coefficient can really carry.
    let geo = (kernel_ns.iter().map(|n| n.ln()).sum::<f64>() / kernel_ns.len() as f64).exp();
    let lo = kernel_ns.iter().copied().fold(f64::MAX, f64::min);
    let hi = kernel_ns.iter().copied().fold(0.0f64, f64::max);
    println!(
        "\nkernel geomean {geo:.4} ns/B over {} patterns (range {lo:.4}..{hi:.4})",
        kernel_ns.len()
    );

    sizes(&docs);
}

/// Seconds spent projecting the DFA, harvesting the lattice, and predicting
/// selectivity — the three phases `Sieve::with` runs in sequence. Timed through the
/// same public entry points a caller has, so a phase cannot be measured cheaper than
/// it is actually invoked.
fn phases(pattern: &str) -> (f64, f64, f64) {
    let dfa = dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .expect("pattern builds");
    let project = fastest(ROUNDS, || {
        std::hint::black_box(sheng::Projection::of(&dfa).is_ok());
    });
    let core = sheng::Projection::of(&dfa).expect("projects");
    let harvest = fastest(ROUNDS, || {
        std::hint::black_box(sheng::harvest(&core).len());
    });
    let quotients = sheng::harvest(&core);
    let select = fastest(ROUNDS, || {
        std::hint::black_box(sheng::worst_case(&quotients, &sheng::prior::DEFAULT_CHAINS));
    });
    (project, harvest, select)
}

/// The same kernel against synthetic document lengths cut from real bytes.
///
/// A refutation filter is judged at whole-document grain, and the vector path needs
/// a superblock to fill before it beats the scalar walk — so a table that only ever
/// reported one length would hide exactly the crossover a caller with small files
/// lands on.
fn sizes(docs: &[Vec<u8>]) {
    let Ok(sieve) = Sieve::ungated(r"(?-u)[A-Z][a-z]+Service") else {
        return;
    };
    let flat: Vec<u8> = docs.iter().flatten().copied().take(8 << 20).collect();
    println!("\n{:>10} {:>12}", "doc bytes", "ns/B");
    for len in [64usize, 256, 1024, 4096, 65536] {
        let cut: Vec<&[u8]> = flat.chunks_exact(len).collect();
        let total = cut.len() * len;
        let secs = fastest(ROUNDS, || {
            for hay in &cut {
                std::hint::black_box(sieve.refutes(hay));
            }
        });
        println!("{len:>10} {:>12.4}", secs * 1e9 / total as f64);
    }
}

fn per_byte(docs: &[Vec<u8>], mut run: impl FnMut(&[u8])) -> f64 {
    let bytes: usize = docs.iter().map(Vec::len).sum();
    let secs = fastest(ROUNDS, || {
        for doc in docs {
            run(doc);
        }
    });
    secs * 1e9 / bytes as f64
}

/// Minimum of `rounds`, because the fastest observed run is the one least polluted by
/// whatever else this machine is doing — and this machine routinely has ten coworker
/// agents on it.
fn fastest(rounds: usize, mut run: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..rounds {
        let t = Instant::now();
        run();
        best = best.min(t.elapsed().as_secs_f64());
    }
    best
}
