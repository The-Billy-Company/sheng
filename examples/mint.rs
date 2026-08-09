//! Mint the calibration constants. Paste the output into `src/prior/minted.rs` and
//! `src/price/minted.rs`; the row it prints carries its own machine and date.
//!
//! Runs from anywhere — the corpus is found by [`common::root`], and `$SHENG_CORPUS`
//! points it at the bytes you actually search rather than the tree this file sits in.
//! Both halves of the output are claims about a *place*: the price row describes one
//! (architecture, kernel) pair, and the persistence matrix describes one corpus.
//! Minting on new silicon adds a row to `price::MINTED`; minting on a new corpus adds
//! one to `prior::DEFAULT_CHAINS`, or replaces a prior a caller passes in a `Policy`.
//!
//! Which is why a run is one claim or the other, never both. Unargued, the mint
//! measures the source prior and then prices every kernel present. Given a prior's
//! name it measures that corpus and stops — because a price row swept over prose is
//! still keyed on (architecture, kernel), so pasting one would overwrite a row
//! measured over the bytes its callers actually search:
//!
//! ```text
//! cargo run --release --example mint                       # SOURCE, plus a row per kernel
//! SHENG_CORPUS=/tmp/gutenberg SHENG_KINDS=txt \
//!     cargo run --release --example mint -- prose          # PROSE alone
//! ```
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

use std::time::Instant;

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use sheng::MAX_CONJUNCTS;
use sheng::prior::{CLASSES, Class};

mod common;

/// Enough real text that the rare-class rows are not a handful of samples — and, for
/// the price half, far enough past any last-level cache that no traversal is ever warm.
const WANT_BYTES: usize = 64 << 20;

/// The corpus the cache-resident column is timed over: small enough to sit inside the
/// L2 of every machine `price::MINTED` names, so `min`-of-[`ROUNDS`] is necessarily
/// taken from a pass whose bytes were already resident.
///
/// That is the whole trick, and it is why the two columns need no separate machinery.
/// A minimum over several traversals discards the cold first pass by construction. Over
/// 64 MiB no pass is ever warm, so the minimum is a memory-resident measurement; over
/// 1 MiB every pass after the first is warm, so the minimum is a cache-resident one.
/// Same timer, same loop, same slate — the corpus size is the independent variable.
const CACHE_BYTES: usize = 1 << 20;

/// The corpus volume under which a run cannot claim to have measured memory at all.
///
/// Past the last-level cache of every machine `price::MINTED` names, with margin. Not a
/// preference: a mint handed less than this measures cache twice and prints a row that
/// says the memory system does not exist.
const MEMORY_FLOOR: usize = 32 << 20;

const ROUNDS: usize = 7;

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

fn main() {
    // The name of the prior being minted, and — because a run is a claim about either
    // a corpus or a machine and never both — which halves run at all. Named, the mint
    // measures that corpus and stops: a price row swept over prose is still keyed on
    // (architecture, kernel), so pasting one would overwrite a row that was measured
    // over the bytes its callers really search.
    let named = std::env::args().nth(1);
    let prior = named
        .as_deref()
        .unwrap_or("SOURCE")
        .trim()
        .to_ascii_uppercase();
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

    persistence(&docs, &prior);
    let freq = histogram(&docs);
    byte_table(&freq, &prior);
    if named.is_some() {
        println!("// prior {prior} only — paste both constants into src/prior/minted.rs.");
        return;
    }

    // A tree smaller than the memory-resident budget cannot produce a memory-resident
    // column, and the failure is silent rather than loud: `corpus_bytes` returns
    // everything it found, so both requests come back holding the *same bytes* and the
    // two columns agree to four decimal places. That reads exactly like the finding
    // "residency does not matter on this machine" — which is how it was in fact read,
    // for an afternoon, off the 0.5 MiB tree this file sits in.
    //
    // So it is a hard refusal rather than a warning. A row is a claim about a memory
    // system, and a mint that never reached memory has no business printing one.
    assert!(
        total > MEMORY_FLOOR,
        "the corpus at {} is {:.2} MiB, under the {} MiB a memory-resident column needs \
         — both columns would hold the same bytes and agree by construction. Aim \
         $SHENG_CORPUS at a tree larger than any last-level cache.",
        common::root().display(),
        total as f64 / (1 << 20) as f64,
        MEMORY_FLOOR >> 20
    );

    // The cache-resident corpus gets its own histogram, and it has to. Both excursion
    // solves invert a cost formula in which the escape *rate* is a known — so feeding
    // a 1 MiB timing the 64 MiB corpus's marginals would attribute the difference
    // between two corpora to the memory system.
    let hot = common::corpus_bytes(CACHE_BYTES);
    let hot_freq = histogram(&hot);
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
    let (cache, memory) = (
        Corpus {
            docs: &hot,
            freq: &hot_freq,
        },
        Corpus {
            docs: &docs,
            freq: &freq,
        },
    );

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
        price(cache, memory);
    }
}

/// A corpus and the byte marginals measured over *it*, kept together because every
/// excursion solve reads both and pairing them wrongly is a silent bias rather than an
/// error.
#[derive(Clone, Copy)]
struct Corpus<'a> {
    docs: &'a [Vec<u8>],
    freq: &'a [f64; 256],
}

/// Marginal frequency of every byte value.
///
/// The escape-set model needs **per-byte** resolution, not per-class: within `Lower`,
/// `a` is several times commoner than `f`, and pricing them alike is the difference
/// between arming a 1.4x winner and arming a 0.44x loser. The class chain still
/// carries the persistence structure; this carries the marginals.
fn histogram(docs: &[Vec<u8>]) -> [f64; 256] {
    let mut n = [0u64; 256];
    for doc in docs {
        for &b in doc {
            n[usize::from(b)] += 1;
        }
    }
    let total: u64 = n.iter().sum();
    n.map(|count| ratio(count, total))
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
        let thin = (n < SUPPORT)
            .then(|| format!(" — {n} pairs is under the support floor: absorbing"))
            .unwrap_or_default();
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
/// three are measured once. Which is which is a claim about what each loop waits on, and
/// it is asserted below rather than assumed:
///
/// * `dfa_skip` and both excursion coefficients reach memory, so they carry a regime.
/// * `dfa_walk` waits on L1 for a table it has already pulled in, and `sieve` is
///   issue-bound at three operations a byte. Neither has any headroom a hotter haystack
///   could give it, so measuring them twice would only publish this timer's noise as a
///   physical effect.
fn price(cache: Corpus<'_>, memory: Corpus<'_>) {
    // Two patterns over the same real bytes, differing only in whether the engine
    // can skip. Both trail a NUL pair, which source text does not contain, so
    // neither can match and both time a full traversal rather than an early exit.
    //
    // The lead is what selects the arm: one rare byte leaves an escape set of one,
    // which the engine accelerates; a 52-byte class is far over its threshold, so it
    // is committed to the walk.
    //
    // The walk is timed over the memory-resident corpus and used for both columns. It
    // is the coefficient the regime does not move, and taking it from the larger corpus
    // keeps it comparable to every row minted before residency existed.
    let walk = timed(memory.docs, WALK_REF);

    let mut skip = [0.0; 2];
    let mut excursions = [0.0; 2];
    let mut skip_e = [[0.0; 2]; 2];
    for (at, corpus) in [(0, cache), (1, memory)] {
        let name = if at == 0 { "Cache" } else { "Memory" };
        println!("\n// ── {name}-resident ──");
        skip[at] = timed(corpus.docs, SKIP_REF);
        excursions[at] = excursion(corpus.docs, corpus.freq);
        // An instrument the slate could not exercise inherits the engine's coefficient
        // rather than a guess — see `skip_excursion`.
        let per = skip_excursion(corpus.docs, corpus.freq);
        for (slot, solved) in per.iter().enumerate() {
            skip_e[slot][at] = if solved.is_nan() {
                excursions[at]
            } else {
                *solved
            };
        }
    }

    // The direction is physics, not a fit, so a run that came out the other way measured
    // something else — a corpus that did not fit, or a machine that was not idle. Said
    // here, where the numbers are, rather than left for the test that reads the pasted
    // row much later.
    //
    // Only the engine's two coefficients are checked. `skip_excursion` re-enters a
    // sixteen-block quotient resident in either regime, so it is not predicted to move,
    // and it is a maximum over a five-pattern slate — warning on its noise would train a
    // reader to ignore this line.
    for (label, hot, cold) in [
        ("dfa_skip", skip[0], skip[1]),
        ("dfa_excursion", excursions[0], excursions[1]),
    ] {
        if hot > cold {
            println!(
                "// WARNING {label}: cache-resident {hot:.4} exceeds memory-resident \
                 {cold:.4} — re-run on an idle machine before pasting this row."
            );
        }
    }

    // Every sieve timing lands before a single line of the constant is printed, because
    // both this and `sieve_ns` narrate what they measured and interleaving the two would
    // hand back a constant with diagnostics inside its array literal.
    // Never a zero from a real measurement: a free pre-pass passes every worth test.
    let sieve: Vec<String> = (1..=MAX_CONJUNCTS)
        .map(|n| match sieve_ns(memory.docs, n) {
            Some(ns) => format!("{ns:.6}"),
            None => "0.0".into(),
        })
        .collect();

    // Emitted as a named row rather than as `ACTIVE`: a calibration belongs to the
    // machine class that produced it, and `price::active()` resolves the running target
    // against `MINTED`. Name it for the target triple, add it to that slice.
    println!("\npub const {}: Calibration = Calibration {{", row_name());
    println!("    arch: {:?},", std::env::consts::ARCH);
    println!("    kernel: Kernel::{:?},", sheng::shuffle::kernel());
    println!("    host: {:?},", common::host());
    println!("    minted: {:?},", common::today());
    // Regime-indexed literals are written `[cache, memory]`, matching
    // `Residency::Cache as usize == 0`.
    println!("    dfa_skip: [{:.6}, {:.6}],", skip[0], skip[1]);
    println!("    dfa_walk: {walk:.6},");
    println!(
        "    dfa_excursion: [{:.6}, {:.6}],",
        excursions[0], excursions[1]
    );
    println!(
        "    skip_excursion: [[{:.6}, {:.6}], [{:.6}, {:.6}]],",
        skip_e[0][0], skip_e[0][1], skip_e[1][0], skip_e[1][1]
    );
    println!("    sieve: [{}],", sieve.join(", "));
    println!("}};");
    println!(
        "// then add {} to price::MINTED — a row nobody lists is a row nobody uses.",
        row_name()
    );
}

/// `LINUX_X86_64_SSSE3`-shaped, from the target and the resolved kernel, so the
/// constant a mint prints cannot be labeled with a machine — or a kernel — other than
/// the one that ran it.
///
/// The kernel is in the name because it is half the key `price::MINTED` is looked up
/// by. Two rows off the same x86_64 box differ in nothing else, and a pair of
/// `LINUX_X86_64`s would be an invitation to paste one over the other.
fn row_name() -> String {
    format!(
        "{}_{}_{}",
        std::env::consts::OS.to_uppercase(),
        std::env::consts::ARCH.to_uppercase(),
        format!("{:?}", sheng::shuffle::kernel()).to_uppercase()
    )
}

/// How many bytes an accelerated engine is charged at walk price per escape byte.
///
/// Solved rather than assumed: time an accelerated pattern whose lead byte trips the
/// skip, then invert the blend `measured = skip*(1-p) + walk*p*E` for `E`. The escape
/// frequency `p` is read from the **same per-byte table the gate uses**, so the solver
/// and the model cannot disagree about what `p` means.
///
/// Eleven lead bytes spanning four classes and two orders of magnitude of frequency
/// are solved independently, so the coefficient is not a fit to any one letter — and
/// the spread across them is the honest measure of how much this single number can
/// really carry.
///
/// Each solution's two baselines are re-timed [`paired`] with the pattern they
/// normalize rather than read from one measurement at the top of the mint, because
/// they are minutes and a varying machine apart otherwise. See [`paired`].
fn excursion(docs: &[Vec<u8>], freq: &[f64; 256]) -> f64 {
    let mut solved = Vec::new();
    println!("// lead   p          ns/B     E");
    for lead in ['e', 't', 'a', 'o', 's', 'f', 'p', 'E', '3', '=', '.'] {
        let p = freq[lead as usize];
        let [ns, skip, walk] = paired(
            docs,
            &mut [
                &mut searcher(&format!(r"(?-u){}\x00\x01zz", regex_escape(lead))),
                &mut searcher(SKIP_REF),
                &mut searcher(WALK_REF),
            ],
        )[..] else {
            unreachable!("three loops in, three timings out")
        };
        let e = (ns - skip * (1.0 - p)) / (walk * p);
        if e.is_finite() && e > 0.0 {
            solved.push(e);
        }
        println!("// {lead:<6} {p:.8} {ns:7.4}  {e:6.2}");
    }
    let mean = solved.iter().sum::<f64>() / solved.len() as f64;
    let lo = solved.iter().copied().fold(f64::MAX, f64::min);
    let hi = solved.iter().copied().fold(0.0f64, f64::max);
    println!(
        "// excursion over {} lead bytes: mean {mean:.3}, range {lo:.2}..{hi:.2}",
        solved.len()
    );
    mean
}

/// The same inversion for the sieve's own skip loop, once per instrument.
///
/// Identical method to [`excursion`] and deliberately so — time a pattern whose
/// quotient start block escapes on a known set, then solve
/// `measured = skip*(1-p) + walk*p*E` for `E` with `p` read from the gate's own byte
/// table. What changes is only *whose* loop is being timed, which is the entire
/// reason the coefficient has to be minted separately: the engine's excursion
/// re-enters a dense DFA and restarts `memchr`, the sieve's re-enters sixteen blocks
/// already in L1.
///
/// Both instruments are swept over sets spanning two orders of magnitude of escape
/// frequency, because a coefficient fitted to one character class is a fit, not a
/// measurement — and the spread printed beside each mean is what says how much a
/// single number can carry.
fn skip_excursion(docs: &[Vec<u8>], freq: &[f64; 256]) -> [f64; 2] {
    // Each entry escapes its start block on exactly the set its name suggests, so
    // `p` is the summed frequency of that set and nothing else.
    const FEW: &[&str] = &[
        r"(?-u)e\x00\x01zz",
        r"(?-u)a\x00\x01zz",
        r"(?-u)p\x00\x01zz",
        r"(?-u)E\x00\x01zz",
        r"(?-u)(alpha|beta|gamma)\x00\x01zz",
    ];
    const WIDE: &[&str] = &[
        r"(?-u)[0-9]\x00\x01zz",
        r"(?-u)[A-Z]\x00\x01zz",
        r"(?-u)[aeiou]\x00\x01zz",
        r"(?-u)[0-9a-fA-F]\x00\x01zz",
        r"(?-u)[.,;:(){}]\x00\x01zz",
    ];
    let mut solved = [0.0f64; 2];
    for (slot, slate) in [FEW, WIDE].iter().enumerate() {
        println!("// skip instrument={slot}   p          ns/B     E");
        let mut each = Vec::new();
        for pattern in *slate {
            let Some((quotient, probe)) = harvest_skip(pattern) else {
                println!("// {pattern:<28} no skip");
                continue;
            };
            if probe.instrument() as usize != slot {
                continue;
            }
            let p: f64 = probe
                .leaves()
                .iter()
                .map(|&b| freq[usize::from(b)])
                .sum::<f64>()
                .clamp(0.0, 1.0);
            let [ns, skip, walk] = paired(
                docs,
                &mut [
                    &mut |hay: &[u8]| {
                        std::hint::black_box(sheng::shuffle::refutes_skipping(
                            &quotient, &probe, hay,
                        ));
                    },
                    &mut searcher(SKIP_REF),
                    &mut searcher(WALK_REF),
                ],
            )[..] else {
                unreachable!("three loops in, three timings out")
            };
            let e = (ns - skip * (1.0 - p)) / (walk * p);
            if e.is_finite() && e > 0.0 {
                each.push(e);
            }
            println!("// {pattern:<28} {p:.8} {ns:7.4}  {e:6.2}");
        }
        // An instrument nothing on the slate exercised keeps the engine's own
        // coefficient: pessimistic, and the direction that declines a skip rather
        // than electing one nobody measured.
        solved[slot] = if each.is_empty() {
            println!("// instrument={slot}: nothing measured — falling back to dfa_excursion");
            f64::NAN
        } else {
            let mean = each.iter().sum::<f64>() / each.len() as f64;
            let lo = each.iter().copied().fold(f64::MAX, f64::min);
            let hi = each.iter().copied().fold(0.0f64, f64::max);
            println!(
                "// skip excursion instrument={slot} over {} sets: worst {hi:.3} (mean {mean:.3}, range {lo:.2}..{hi:.2})",
                each.len()
            );
            // The **worst** solution, where `dfa_excursion` beside it takes the mean,
            // and the asymmetry is deliberate. That coefficient prices the rival, so
            // erring high there would flatter the sieve; this one prices the sieve,
            // where erring high can only decline a skip. It also covers what the
            // model structurally cannot see: excursion *length* is a property of the
            // quotient, not of the escape frequency, and a literal like `panic!\(`
            // walks further after its lead byte than anything on this slate. At the
            // mean, that pattern priced below the composition kernel it measures 1.5x
            // slower than — the max declines it correctly.
            hi
        };
    }
    solved
}

/// The first harvested quotient for `pattern` and the skip over its start block, or
/// `None` when the pattern yields neither.
fn harvest_skip(pattern: &str) -> Option<(sheng::Quotient, sheng::Skip)> {
    let dfa = matcher(pattern);
    let core = sheng::Projection::of(&dfa).ok()?;
    let quotient = sheng::harvest(&core).into_iter().next()?;
    let probe = sheng::Skip::of(&quotient.rows, quotient.start)?;
    (quotient.start < quotient.threshold).then_some((quotient, probe))
}

fn regex_escape(c: char) -> String {
    if c.is_ascii_alphanumeric() {
        c.to_string()
    } else {
        format!("\\{c}")
    }
}

fn matcher(pattern: &str) -> dense::DFA<Vec<u32>> {
    dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .expect("pattern builds")
}

/// `timed` without the banner, for the sweep.
fn quiet(docs: &[Vec<u8>], pattern: &str) -> f64 {
    let dfa = matcher(pattern);
    per_byte(docs, |hay| {
        assert!(
            dfa.try_search_fwd(&Input::new(hay))
                .expect("no quit")
                .is_none(),
            "calibration pattern must not match real source"
        );
    })
}

fn timed(docs: &[Vec<u8>], pattern: &str) -> f64 {
    let dfa = matcher(pattern);
    let start = dfa
        .start_state_forward(&Input::new(b""))
        .expect("start state");
    let accel = !dfa.accelerator(start).is_empty();
    let ns = quiet(docs, pattern);
    println!("// {pattern:?} accel={accel} → {ns:.4} ns/B");
    ns
}

/// The **composition kernel's** price at `n` conjuncts, or `None` when no pattern on
/// the slate harvests exactly that many — an unmeasured coefficient must read as
/// infinity downstream, never as free.
///
/// Built with `skip: false` throughout, and that is not a detail. This coefficient is
/// the number a candidate skip is compared against in `Lane::plan`, so a mint that
/// let its own timings take the skip path would be setting the exchange rate in the
/// currency it was measuring — the fast patterns would elect the skip, drag the
/// coefficient down, and the next build would find fewer skips worth taking.
fn sieve_ns(docs: &[Vec<u8>], n: usize) -> Option<f64> {
    const SLATE: &[&str] = &[
        r"(?-u)WalletService",
        r"(?-u)a[^\n]*b",
        r"(?-u)(alpha|beta|gamma)",
        r"(?-u)[A-Z][a-z]+Service",
        r"(?-u)[0-9]{3}-[0-9]{4}",
        r"(?-u)<[^>]*>",
        r"(?-u)ab+c",
    ];
    // The regime is immaterial here and stated anyway: `Gate::Ungated` consults no
    // price, and `skip: false` is what pins every lane to the composition kernel whose
    // cost this function exists to measure.
    let composing = sheng::Policy {
        gate: sheng::Gate::Ungated,
        skip: false,
        ..sheng::Policy::new(sheng::price::Residency::Memory)
    };
    let build = |p: &&str| sheng::Sieve::with(p, &composing).ok();
    let harvested: Vec<(&str, usize)> = SLATE
        .iter()
        .filter_map(|p| build(p).map(|s| (*p, s.conjuncts())))
        .collect();
    if n == 1 {
        println!("// slate conjunct census: {harvested:?}");
    }
    let sieve = SLATE
        .iter()
        .filter_map(build)
        .find(|s| s.conjuncts() == n)?;
    let ns = per_byte(docs, |hay| {
        std::hint::black_box(sieve.refutes(hay));
    });
    println!("// sieve conjuncts={n} → {ns:.4} ns/B");
    Some(ns)
}

/// One rare byte leaves an escape set of one, which the engine accelerates.
const SKIP_REF: &str = r"(?-u)\x00\x01zz";
/// A 52-byte class is far over the engine's accelerator threshold, so it walks.
const WALK_REF: &str = r"(?-u)[A-Za-z]\x00\x01zz";

/// A closure that runs one full engine search, for use as a paired baseline.
fn searcher(pattern: &str) -> impl FnMut(&[u8]) {
    let dfa = matcher(pattern);
    move |hay: &[u8]| {
        assert!(
            dfa.try_search_fwd(&Input::new(hay))
                .expect("no quit")
                .is_none(),
            "calibration pattern must not match real source"
        );
    }
}

/// One leg of a paired measurement: a loop to sweep a corpus with.
type Leg<'a> = &'a mut dyn FnMut(&[u8]);

/// Time several loops **interleaved**, one traversal of each per round, and hand back
/// each one's own minimum.
///
/// A ratio is only a measurement when its numerator and denominator saw the same
/// machine. This laptop runs at load average 12 with ten coworker agents on it, and
/// contention does not fall equally on every loop — a branchy excursion degrades
/// further under it than a streaming `memchr` does. Inverting an excursion timed now
/// against a baseline timed several minutes ago therefore does not measure the
/// excursion; it measures the drift between two afternoons. Taken interleaved, both
/// legs meet the same contention in the same round and the quotient survives it: the
/// same sweep that read 5.33 and 9.08 on consecutive unpaired runs holds still here.
fn paired(docs: &[Vec<u8>], runs: &mut [Leg<'_>]) -> Vec<f64> {
    let bytes: usize = docs.iter().map(Vec::len).sum();
    let mut best = vec![f64::MAX; runs.len()];
    for _ in 0..ROUNDS {
        for (slot, run) in runs.iter_mut().enumerate() {
            let t = Instant::now();
            for doc in docs {
                run(doc);
            }
            best[slot] = best[slot].min(t.elapsed().as_secs_f64());
        }
    }
    best.iter().map(|secs| secs * 1e9 / bytes as f64).collect()
}

fn per_byte(docs: &[Vec<u8>], mut run: impl FnMut(&[u8])) -> f64 {
    let bytes: usize = docs.iter().map(Vec::len).sum();
    let mut best = f64::MAX;
    for _ in 0..ROUNDS {
        let t = Instant::now();
        for doc in docs {
            run(doc);
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    best * 1e9 / bytes as f64
}

fn ratio(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        num as f64 / den as f64
    }
}
