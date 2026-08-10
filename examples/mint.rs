//! Mint the calibration constants. Paste the output into `src/prior/minted.rs` and
//! `src/price/minted.rs`; the row it prints carries its own machine and date.
//!
//! Runs from anywhere — the corpus is found by [`common::root`], and `$SHENG_CORPUS`
//! points it at the bytes you actually search rather than the tree this file sits in.
//! Both halves of the output are claims about a *place*: the price row describes one
//! (operating system, architecture, kernel) triple, and the persistence matrix
//! describes one corpus.
//! Minting on new silicon adds a row to `price::MINTED`; minting on a new corpus adds
//! one to `prior::DEFAULT_CHAINS`, or replaces a prior a caller passes in a `Policy`.
//!
//! Which is why a run is one claim or the other, never both. Unargued, the mint
//! measures the source prior and then prices every kernel present. Given a prior's
//! name it measures that corpus and stops — because a price row swept over prose is
//! still keyed on its machine triple, so pasting one would overwrite a row
//! measured over the bytes its callers actually search:
//!
//! ```text
//! cargo run --release --example mint                       # SOURCE, plus a row per kernel
//! SHENG_CORPUS=/tmp/gutenberg SHENG_KINDS=txt \
//!     cargo run --release --example mint -- prose          # PROSE alone
//! SHENG_CORPUS=/ci/corpus cargo run --release --example mint -- price   # rows alone
//! SHENG_CORPUS=/ci/corpus cargo run --release --example mint -- corpus  # is it big enough?
//! ```
//!
//! `price` names no prior; it is the *other* half asked for alone, and it exists for the
//! same reason a prior can be. A calibration row has to be minted over a corpus big
//! enough to reach main memory, which on a CI runner means bytes nobody here chose as a
//! byte process — so the prior half of that run would be a claim about a corpus that is
//! not any of the four shipped ones, printed in a form that looks exactly like something
//! to paste. `.github/workflows/mint.yml` therefore asks for the half it came for.
//!
//! `corpus` asks for neither half. It walks the tree, prints the banner, applies the
//! [`MEMORY_FLOOR`] refusal and stops — the precondition of a price row, separated from
//! the row so it can be checked before the measurement instead of during it. A CI leg
//! that assembled too small a corpus should learn that in seconds rather than after
//! paying for a persistence sweep it will throw away, and because the gate walks the
//! corpus with the same code the run does, it cannot pass a tree the run then rejects.
//!
//! `$SHENG_KINDS` is not optional for that second form: a tree of prose, JSON, or logs
//! is invisible to the source-extension default however `$SHENG_CORPUS` is aimed, and
//! that omission is the whole reason every shipped prior once described a code tree.
//! The corpora the shipped non-source priors were minted from are pinned by commit in
//! `.github/workflows/priors.yml`, which re-mints them and fails on drift.
//!
//! One run prints a price row per kernel the machine can execute, because a row is
//! keyed on the kernel and one x86_64 box holds three of them. That sweep is not a
//! convenience: `arch::kernel` refuses to dispatch to a kernel `price::MINTED` has no
//! row for, so a mint that only ever measured what dispatch chose could never reach a
//! newly added instruction set — the row it needs would be waiting on the measurement
//! the measurement was waiting on. `shuffle::force` breaks that circle, and validates
//! against the same runtime probe, so nothing here can time silicon that is not present.
//!
//! Two independent measurements, because a measured value with no machine beside
//! it is an anecdote:
//!
//! * **persistence** — the first-order transition matrix over byte classes. The
//!   memoryless prior it replaces prices a `k`-byte class run as `p^k`, which is
//!   wrong by the class's persistence ratio raised to `k`.
//! * **price** — nanoseconds per byte for the sieve kernel at each conjunct count
//!   and for the rival engine both with and without its start-state accelerator.
//!   Nanoseconds, not cycles: the arming inequality is scale-invariant as long as
//!   both sides share a unit, so measuring time directly costs one fewer assumed
//!   constant (the clock frequency) than converting to cycles would.

use sheng::price::{Bench, Calibration, Report, Residency, histogram};
use sheng::prior::{CLASSES, Class};

mod common;

/// Enough real text that the rare-class rows are not a handful of samples — and, for
/// the price half, far enough past any last-level cache that no traversal is ever warm.
const WANT_BYTES: usize = 64 << 20;

/// The corpus the cache-resident column is timed over: small enough to sit inside the
/// L2 of every machine `price::MINTED` names, so the minimum over
/// [`Bench::rounds`](sheng::price::Bench::rounds) is necessarily taken from a pass whose
/// bytes were already resident.
///
/// That is the whole trick, and it is why the two columns need no separate machinery.
/// A minimum over several traversals discards the cold first pass by construction. Over
/// 64 MiB no pass is ever warm, so the minimum is a memory-resident measurement; over
/// 1 MiB every pass after the first is warm, so the minimum is a cache-resident one.
/// Same timer, same loop, same slate — the corpus size is the independent variable, which
/// is why `Bench` infers the regime it is filling from the sample it was handed rather
/// than being told.
const CACHE_BYTES: usize = 1 << 20;

/// The corpus volume under which a run cannot claim to have measured memory at all.
///
/// Past the last-level cache of every machine `price::MINTED` names, with margin. Not a
/// preference: a mint handed less than this measures cache twice and prints a row that
/// says the memory system does not exist.
const MEMORY_FLOOR: usize = 32 << 20;

/// How many observed pairs a transition row needs before it counts as a measurement.
///
/// A budget cannot promise this per row, only in aggregate: a class that barely occurs
/// conditions its row on whatever it got. English prose holds nine non-ASCII bytes in
/// eleven megabytes and the loghub sample holds none at all, which counted straight
/// print a row of ninths and a row of zeros — the first is a coincidence wearing six
/// decimal places, and the second is not a distribution.
///
/// So a row under this floor is not smoothed toward anything. It is written
/// **absorbing** — the class always repeats — which is the most persistent row that
/// exists and therefore prices every run through it at the maximum. Same doctrine as
/// [`price::UNMEASURED`](sheng::price::UNMEASURED): what was not measured has to read
/// as the worst case rather than as a guess, because the guess is what arms a sieve
/// nobody timed.
const SUPPORT: u64 = 1 << 10;

/// The refusal that keeps a price row from describing a cache.
///
/// A tree smaller than [`MEMORY_FLOOR`] cannot produce a memory-resident column, and the
/// failure is silent rather than loud: `corpus_bytes` returns everything it found, so both
/// requests come back holding the *same bytes* and the two columns agree to four decimal
/// places. That reads exactly like the finding "residency does not matter on this
/// machine" — which is how it was in fact read, for an afternoon, off the 0.5 MiB tree
/// this file sits in.
///
/// So it is a hard refusal rather than a warning. A row is a claim about a memory system,
/// and a mint that never reached memory has no business printing one.
fn memory_resident(total: usize) {
    assert!(
        total > MEMORY_FLOOR,
        "the corpus at {} is {:.2} MiB, under the {} MiB a memory-resident column needs \
         — both columns would hold the same bytes and agree by construction. Aim \
         $SHENG_CORPUS at a tree larger than any last-level cache.",
        common::root().display(),
        total as f64 / (1 << 20) as f64,
        MEMORY_FLOOR >> 20
    );
}

fn main() {
    // The name of the prior being minted, and — because a run is a claim about either
    // a corpus or a machine and never both — which halves run at all. Named, the mint
    // measures that corpus and stops: a price row swept over prose is still keyed on
    // its machine triple, so pasting one would overwrite a row that was measured over
    // the bytes its callers really search.
    let named = std::env::args().nth(1);
    let asked = named
        .as_deref()
        .unwrap_or("SOURCE")
        .trim()
        .to_ascii_uppercase();
    // The half of the output this run is a claim about. `PRICE` is not a prior's name —
    // see the module documentation for why a mint aimed at a CI-sized corpus asks for the
    // rows without the prior that corpus would otherwise imply.
    let prices_only = asked == "PRICE";
    // Neither a prior nor a measurement: the corpus check on its own, so the thing that
    // can refuse an hour of a runner's time can be asked *before* the hour. See below.
    let corpus_only = asked == "CORPUS";
    let prior = if prices_only || corpus_only {
        "SOURCE".into()
    } else {
        asked
    };
    let docs = common::corpus_bytes(WANT_BYTES);
    let total: usize = docs.iter().map(Vec::len).sum();
    println!(
        "// minted on {} · {} · {} files · {:.1} MiB from {}\n",
        common::machine(),
        common::today(),
        docs.len(),
        total as f64 / (1 << 20) as f64,
        common::root().display()
    );

    // The floor asked in isolation, which is the whole of `CORPUS` mode. The check below
    // is a *precondition* of a price row, not a finding about one, and leaving it to fire
    // where it fires means a leg that assembled too small a corpus learns so only after
    // paying for the persistence sweep — an assertion is a fine way to refuse to publish
    // and a poor way to spend a runner. Same reading of the same bytes by the same
    // walker, so the gate cannot disagree with the run it gates.
    if corpus_only {
        memory_resident(total);
        // Whether the budget bound before the tree ran out. Both outcomes are valid rows,
        // but only one of them is a claim about the corpus somebody assembled: at the
        // budget the walk stops mid-tree, so what got measured is a deterministic
        // *prefix* — the sort in `common::walk` makes it the same prefix every run, and
        // not the same thing as the tree named in the workflow.
        println!(
            "// corpus admits a memory-resident column{}",
            if total >= WANT_BYTES {
                ", and is larger than the budget — the walk read a prefix of it"
            } else {
                ", entire"
            }
        );
        return;
    }

    if !prices_only {
        persistence(&docs, &prior);
    }
    // The same `histogram` a `Bench` prices its escape sets with, which is what keeps a
    // pasted `*_BYTES` table and a runtime measurement talking about one distribution.
    let freq = histogram(&borrowed(&docs));
    if !prices_only {
        byte_table(&freq, &prior);
        if named.is_some() {
            println!("// prior {prior} only — paste both constants into src/prior/minted.rs.");
            return;
        }
    }

    memory_resident(total);

    // The cache-resident corpus is a separate sample, and each `Bench` computes its own
    // marginals from the bytes it was handed — which it has to. Both excursion solves
    // invert a cost formula in which the escape *rate* is a known, so feeding a 1 MiB
    // timing the 64 MiB corpus's marginals would attribute the difference between two
    // corpora to the memory system.
    let hot = common::corpus_bytes(CACHE_BYTES);
    let hot_bytes: usize = hot.iter().map(Vec::len).sum();
    assert!(
        hot_bytes < total / 8,
        "the cache-resident slice is {:.2} MiB of a {:.2} MiB corpus — too close to the \
         whole thing to be a separate regime",
        hot_bytes as f64 / (1 << 20) as f64,
        total as f64 / (1 << 20) as f64
    );
    println!(
        "// cache-resident column: {} files · {:.2} MiB, its own marginals\n",
        hot.len(),
        hot_bytes as f64 / (1 << 20) as f64
    );
    let (cache, memory) = (borrowed(&hot), borrowed(&docs));

    // One row per kernel this silicon can run, not one for the kernel it would have
    // dispatched to — because `arch::kernel` will not dispatch to a kernel `MINTED`
    // has no row for, so the newest instruction set is always the one a
    // dispatch-following mint could never reach. `force` is the seam that breaks that
    // circle, and it refuses any kernel the probe did not admit, so this loop can only
    // time silicon that is really here.
    let kernels = sheng::shuffle::available();
    println!("// kernels this machine can run, fastest first: {kernels:?}\n");
    for &kernel in kernels {
        assert!(
            sheng::shuffle::force(kernel),
            "{kernel:?} came from available() but force() refused it"
        );
        println!("\n// ── {kernel:?} ─────────────────────────────────────────────");
        price(&cache, &memory);
    }
}

/// The shape `sheng::price::Bench` reads a corpus in.
///
/// Slices rather than `Vec`s, so a caller who already holds their documents in one
/// allocation — a memory map, a concatenated batch — can price them without copying.
/// That is the ordinary case for the audience this API was added for, and the mint pays
/// one pointer-per-file to meet it.
fn borrowed(docs: &[Vec<u8>]) -> Vec<&[u8]> {
    docs.iter().map(Vec::as_slice).collect()
}

fn byte_table(freq: &[f64; 256], prior: &str) {
    println!("pub const {prior}_BYTES: [f64; 256] = [");
    for row in freq.chunks(8) {
        let cells: Vec<String> = row.iter().map(|f| format!("{f:.8}")).collect();
        println!("    {},", cells.join(", "));
    }
    println!("];\n");
}

/// `next[i][j]` — how often class `j` follows class `i`, and the persistence ratio
/// that makes the memoryless prior wrong.
fn persistence(docs: &[Vec<u8>], prior: &str) {
    let mut counts = [[0u64; CLASSES]; CLASSES];
    let mut marginal = [0u64; CLASSES];
    for doc in docs {
        for pair in doc.windows(2) {
            counts[Class::of(pair[0]) as usize][Class::of(pair[1]) as usize] += 1;
        }
        for &b in doc {
            marginal[Class::of(b) as usize] += 1;
        }
    }

    let grand: u64 = marginal.iter().sum();
    let support = counts.map(|row| row.iter().sum::<u64>());
    // Resolved before anything is printed so the constant and the ratio table below it
    // cannot describe different matrices — the diagnostics read the emitted row, not
    // the counts it came from. Rows the corpus could not speak for absorb; see SUPPORT.
    let next: [[f64; CLASSES]; CLASSES] = std::array::from_fn(|from| {
        std::array::from_fn(|to| match support[from] >= SUPPORT {
            true => ratio(counts[from][to], support[from]),
            false => f64::from(u8::from(from == to)),
        })
    });

    println!("pub const {prior}: Chain = Chain {{");
    println!("    next: [");
    for ((row, class), n) in next.iter().zip(Class::ALL).zip(support) {
        let cells: Vec<String> = row.iter().map(|p| format!("{p:.6}")).collect();
        let thin = if n < SUPPORT {
            format!(" — {n} pairs is under the support floor: absorbing")
        } else {
            String::new()
        };
        println!("        [{}], // {class:?}{thin}", cells.join(", "));
    }
    println!("    ],");
    let start: Vec<String> = marginal
        .iter()
        .map(|&c| format!("{:.6}", ratio(c, grand)))
        .collect();
    println!("    start: [{}],", start.join(", "));
    println!("}};\n");

    println!("// class      marginal  persistent  ratio");
    for ((i, row), &seen) in next.iter().enumerate().zip(&marginal) {
        let m = ratio(seen, grand);
        let p = row[i];
        println!(
            "// {:<10} {m:8.4}  {p:10.4}  {:5.1}x",
            format!("{:?}", Class::ALL[i]),
            if m > 0.0 { p / m } else { 0.0 }
        );
    }
    println!();
}

/// Nanoseconds per byte for each kernel, timed alone so a coefficient can be
/// re-minted without re-deriving any other.
///
/// Two of the five coefficients are measured **twice**, once per `price::Residency`, and
/// three are measured once. Which is which is a claim about what each loop waits on:
///
/// * `dfa_skip` and both excursion coefficients reach memory, so they carry a regime.
/// * `dfa_walk` waits on L1 for a table it has already pulled in, and `sieve` is
///   issue-bound at three operations a byte. Neither has any headroom a hotter haystack
///   could give it, so measuring them twice would only publish this timer's noise as a
///   physical effect.
///
/// # The measurement is the library's, and only the publishing is this file's
///
/// Every timing loop, every reference pattern and every inversion behind the numbers
/// below lives in `sheng::price::Bench`, which is a shipped API precisely so that a
/// machine with no row in `price::MINTED` can take one without this repository
/// ([`price::measure`](sheng::price::Bench)). This file is what turns two of those rows
/// into a pasteable `const` — the *publishing* half, which is the only half that has
/// anything to do with a source file.
///
/// It used to be both halves, and the duplication was a live drift hazard rather than a
/// tidiness complaint: the slate a `sieve` coefficient is timed over and the slate a
/// `Lane::plan` compares a skip against are supposed to be the same slate, and while
/// there were two copies nothing said so.
fn price(cache: &[&[u8]], memory: &[&[u8]]) {
    // One `Bench` per regime, and the regime is inferred from the sample rather than
    // declared: what decides whether a timing loop read cache or memory is how many
    // bytes it swept, which is the one thing the loop's own corpus already knows.
    let taken: Vec<(Residency, Report)> = [cache, memory]
        .into_iter()
        .filter_map(|docs| {
            let bench = Bench::new(docs);
            match bench.report() {
                Ok(report) => Some((report.at, report)),
                Err(why) => {
                    println!("// {:?}-resident column not taken: {why}", bench.regime());
                    None
                },
            }
        })
        .collect();
    for (at, report) in &taken {
        narrate(*at, report);
    }

    // Both columns in one row, or the refusal that says why they cannot be. `merge` is
    // where the cross-regime sanity check lives, because it is the only place both
    // columns exist at once: a cache-resident `dfa_skip` above the memory-resident one
    // is not a finding, it is a machine that was busy, and the crate would rather report
    // `Uncalibrated` for a regime than price it off noise.
    let Some(row) = taken
        .iter()
        .map(|(_, report)| report.calibration)
        .reduce(|a, b| {
            a.merge(&b).unwrap_or_else(|| {
                println!(
                    "// WARNING the cache-resident column prices the engine's skip above \
                     the memory-resident one — a hotter haystack cannot cost more, so that \
                     column measured a busy machine. Withheld; it reads 0.0 and a caller \
                     declaring Residency::Cache will get Uncalibrated."
                );
                Calibration {
                    dfa_skip: [0.0, b.dfa_skip[1]],
                    dfa_excursion: [0.0, b.dfa_excursion[1]],
                    skip_excursion: b.skip_excursion.map(|per| [0.0, per[1]]),
                    ..b
                }
            })
        })
    else {
        println!("// no column could be measured at all — nothing to paste.");
        return;
    };

    // Emitted as a named row rather than as `ACTIVE`: a calibration belongs to the
    // machine class that produced it, and `price::active()` resolves the running target
    // against `MINTED`. Name it for the target triple, add it to that slice.
    println!("\npub const {}: Calibration = Calibration {{", row_name());
    // Printed from the crate's own `cfg`-derived constants rather than from
    // `std::env::consts`, so the row names exactly what `price::active` will match it by.
    println!("    os: {:?},", row.os);
    println!("    arch: {:?},", row.arch);
    println!("    kernel: Kernel::{:?},", row.kernel);
    // The two fields a *runtime* row cannot fill, and the whole reason this file still
    // exists: `host` and `minted` are `&'static str`, so a row taken by `Bench` reports
    // its own provenance as "measured at run time" and leaves naming the silicon and the
    // date to whoever is pasting it.
    println!("    host: {:?},", common::host());
    println!("    minted: {:?},", common::today());
    // Regime-indexed literals are written `[cache, memory]`, matching
    // `Residency::Cache as usize == 0`.
    println!(
        "    dfa_skip: [{:.6}, {:.6}],",
        row.dfa_skip[0], row.dfa_skip[1]
    );
    println!("    dfa_walk: {:.6},", row.dfa_walk);
    println!(
        "    dfa_excursion: [{:.6}, {:.6}],",
        row.dfa_excursion[0], row.dfa_excursion[1]
    );
    println!(
        "    skip_excursion: [[{:.6}, {:.6}], [{:.6}, {:.6}]],",
        row.skip_excursion[0][0],
        row.skip_excursion[0][1],
        row.skip_excursion[1][0],
        row.skip_excursion[1][1]
    );
    let sieve: Vec<String> = row.sieve.iter().map(|ns| format!("{ns:.6}")).collect();
    println!("    sieve: [{}],", sieve.join(", "));
    println!("}};");
    println!(
        "// then add {} to price::MINTED — a row nobody lists is a row nobody uses.",
        row_name()
    );
}

/// The evidence behind one column, printed so a human can judge how well-determined the
/// coefficients they are about to paste actually are.
///
/// A mean with no spread beside it is the kind of number that looks like a measurement
/// and is not, which is why `Report` carries every independent solution rather than only
/// the figure the row was built from.
fn narrate(at: Residency, report: &Report) {
    let row = &report.calibration;
    println!(
        "\n// ── {at:?}-resident · {:.2} MiB ──",
        report.bytes as f64 / (1 << 20) as f64
    );
    // The two coefficients that are read straight off a timer rather than solved, so
    // there is nothing to spread and only the figure to print.
    println!(
        "// dfa_skip {:.4} ns/B (accelerated) · dfa_walk {:.4} ns/B (committed)",
        row.dfa_skip[at as usize], row.dfa_walk
    );
    for (n, ns) in row.sieve.iter().enumerate().filter(|&(_, &ns)| ns > 0.0) {
        println!("// sieve conjuncts={} → {ns:.4} ns/B", n + 1);
    }
    println!("// pattern                       p          ns/B     E");
    let engine = &report.engine;
    for solved in engine {
        println!(
            "// {:<28} {:.8} {:7.4}  {:6.2}",
            solved.pattern, solved.escape, solved.ns, solved.excursion
        );
    }
    if let Some((mean, lo, hi)) = Report::spread(engine) {
        println!(
            "// dfa_excursion over {} lead bytes: mean {mean:.3}, range {lo:.2}..{hi:.2}",
            engine.len()
        );
    } else {
        println!("// dfa_excursion: nothing solved — the row reads the physical floor of 1.0");
    }
    for (slot, each) in report.probes.iter().enumerate() {
        for solved in each {
            println!(
                "// instrument={slot} {:<17} {:.8} {:7.4}  {:6.2}",
                solved.pattern, solved.escape, solved.ns, solved.excursion
            );
        }
        match Report::spread(each) {
            // The **worst** solution is what the row takes, where `dfa_excursion` above
            // takes the mean: this term prices the sieve, so erring high can only decline
            // a skip, while erring high on the engine's would flatter one.
            Some((mean, lo, hi)) => println!(
                "// skip_excursion instrument={slot} over {} sets: worst {hi:.3} \
                 (mean {mean:.3}, range {lo:.2}..{hi:.2})",
                each.len()
            ),
            None => println!(
                "// skip_excursion instrument={slot}: nothing measured — inherits dfa_excursion"
            ),
        }
    }
    println!("// slate conjunct census: {:?}", report.conjuncts);
}

/// `LINUX_X86_64_SSSE3`-shaped, from the target and the resolved kernel, so the
/// constant a mint prints cannot be labeled with a machine — or a kernel — other than
/// the one that ran it.
///
/// All three parts are in the name because all three are the key `price::MINTED` is
/// looked up by. Two rows off the same x86_64 box differ in nothing but the kernel, and a
/// pair of `LINUX_X86_64`s would be an invitation to paste one over the other — and two
/// rows off the same architecture under different operating systems are two machines,
/// which is the distinction that column was added to keep.
fn row_name() -> String {
    format!(
        "{}_{}_{}",
        sheng::price::OS.to_uppercase(),
        sheng::price::ARCH.to_uppercase(),
        format!("{:?}", sheng::shuffle::kernel()).to_uppercase()
    )
}

fn ratio(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        num as f64 / den as f64
    }
}
