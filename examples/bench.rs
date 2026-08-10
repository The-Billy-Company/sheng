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
//! * **build** — microseconds for [`sheng::Sieve::new`], which a caller compiling one
//!   sieve per literal (regulator does) pays once per literal and notices immediately.
//! * **length** — both the kernel and the engine it fronts, swept over record sizes, so
//!   the table reports the sieve's *edge* rather than only its price. That is the
//!   quantity [`sheng::price::VALIDITY_FLOOR`] bounds, and it cannot be read off either
//!   loop alone — see [`sizes`].
//!
//! Ungated throughout: a gated build on a pattern the economics decline would time
//! nothing at all.

use std::time::Instant;

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
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

/// Both loops against synthetic document lengths cut from real bytes — which is the
/// measurement [`sheng::price::VALIDITY_FLOOR`] is a claim about.
///
/// The sieve's own curve alone cannot settle where a verdict stops travelling, and
/// that is worth being exact about, because this table used to report only that curve.
/// The gate compares a *ratio*, and both of its legs move as records shorten — in
/// opposite directions. The sieve pays a per-call cost the model does not carry, so it
/// gets dearer per byte. The rival gets *cheaper* per byte, because consecutive searches
/// over short records are independent dependency chains that a wide core overlaps and a
/// single long walk cannot. Both errors push the same way: they flatter the sieve.
///
/// So what is printed is the edge, and the edge relative to the length every coefficient
/// was minted at. That last column is the model's error at each length, and where it
/// crosses [`sheng::price::MARGIN`] is where the floor belongs.
fn sizes(docs: &[Vec<u8>]) {
    let Ok(sieve) = Sieve::ungated(r"(?-u)[A-Z][a-z]+Service") else {
        return;
    };
    // Never present in source text, so both legs time a full traversal rather than an
    // early exit. The first has a 52-byte class ahead of the sentinel, far over the
    // engine's accelerator threshold, so it walks; the second leads with the rare byte
    // and streams. Together they are the two rivals the gate ever prices against.
    let mut walk = searcher(r"(?-u)[A-Za-z]\x00\x01zz");
    let mut skip = searcher(r"(?-u)\x00\x01zz");

    let flat: Vec<u8> = docs.iter().flatten().copied().take(8 << 20).collect();
    let mut at = |len: usize| {
        let cut: Vec<&[u8]> = flat.chunks_exact(len).collect();
        let total = (cut.len() * len) as f64;
        let mut best = [f64::MAX; 3];
        // Interleaved: a ratio is a measurement only when both legs saw one machine.
        for _ in 0..ROUNDS {
            for (slot, run) in [
                &mut (|hay: &[u8]| {
                    std::hint::black_box(sieve.refutes(hay));
                }) as &mut dyn FnMut(&[u8]),
                &mut walk,
                &mut skip,
            ]
            .into_iter()
            .enumerate()
            {
                let t = Instant::now();
                for hay in &cut {
                    run(hay);
                }
                best[slot] = best[slot].min(t.elapsed().as_secs_f64());
            }
        }
        best.map(|secs| secs * 1e9 / total)
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let nominal = sheng::price::NOMINAL_LEN as usize;
    let [base_sieve, base_walk, _] = at(nominal);
    let minted = base_walk / base_sieve;

    println!(
        "\n{:>10} {:>10} {:>10} {:>10} {:>8} {:>9}",
        "doc bytes", "sieve ns/B", "walk ns/B", "skip ns/B", "edge", "vs minted"
    );
    for len in [64usize, 128, 256, 512, 1024, 2048, 4096, 16384, 65536] {
        let [ns, walk, skip] = at(len);
        let edge = walk / ns;
        let floor = len as f64 == sheng::price::VALIDITY_FLOOR;
        println!(
            "{len:>10} {ns:>10.4} {walk:>10.4} {skip:>10.4} {edge:>8.2} {:>8.1}%{}",
            (edge / minted - 1.0) * 100.0,
            if floor { "  <- VALIDITY_FLOOR" } else { "" }
        );
    }
    // The nominal row is a *second* measurement of the same baseline, so whatever it
    // reports is this sweep's own repeatability — and every other row has to be read
    // against that rather than against zero.
    println!(
        "  the model prices this sieve length-free, at the {minted:.2}x it measures over \
         {nominal}-byte records;\n  a row further from that than {:.0}% is a verdict MARGIN no \
         longer covers, and the {nominal} row\n  is a re-measurement of the baseline, so its \
         own deviation is this sweep's noise.",
        sheng::price::MARGIN * 100.0
    );
}

/// A search loop over one pattern, for use as a timing leg.
fn searcher(pattern: &str) -> impl FnMut(&[u8]) {
    let dfa = dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .expect("the reference pattern builds");
    move |hay: &[u8]| {
        std::hint::black_box(dfa.try_search_fwd(&Input::new(hay)).expect("no quit bytes"));
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
