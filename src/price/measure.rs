//! Self-pricing: a machine measures its **own** calibration row, over the caller's own
//! bytes, instead of waiting for someone to ship one for it.
//!
//! # The reach problem this exists to solve
//!
//! [`MINTED`](super::MINTED) is a finite list, and a triple absent from it resolves to
//! [`UNMEASURED`](super::UNMEASURED) and declines every pattern — deliberately, because
//! inheriting another machine's optimism is the one failure a calibration exists to
//! prevent. But the consequence is that `riscv64`, `powerpc64`, `s390x` and
//! `loongarch64` are **inert**: every kernel in this crate compiles and runs there (the
//! scalar composition pass is portable by construction), and the gate refuses all of it
//! for want of five coefficients nobody has taken.
//!
//! Which is a bad trade, because those coefficients are not hard to take and the machine
//! that needs them is *already running*. So the measurement stops being a repository
//! ceremony — clone, `cargo run --example mint`, paste a `const`, open a pull request,
//! wait for a release — and becomes a call:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use sheng::price::Calibration;
//! use sheng::{Policy, Screen};
//!
//! # let corpus: Vec<Vec<u8>> = Vec::new();
//! // A sample of the documents this process is really going to search.
//! let sample: Vec<&[u8]> = corpus.iter().map(Vec::as_slice).collect();
//! let mine = Calibration::measure(&sample)?;
//!
//! let mut policy = Policy::new(mine.regime().expect("a measured row names one regime"));
//! policy.calibration = mine;
//! let screen = Screen::with(r"(?-u)AKIA[0-9A-Z]{16}", &policy)?;
//! # Ok(()) }
//! ```
//!
//! # What is different about a row taken here
//!
//! Two things, and both make it *better* evidence than a shipped row rather than worse:
//!
//! * It is a claim about **this** machine, not about a machine that shares this
//!   machine's `(os, arch, kernel)` triple. That triple is the finest key a binary can
//!   ask about itself ([`OS`](super::OS)) and it cannot tell an M-series laptop from a
//!   datacenter Ampere.
//! * It is taken over the caller's **own documents**, so the byte marginals the escape
//!   sets are priced under are measured rather than borrowed from one of the four
//!   shipped [`prior`](crate::prior) corpora.
//!
//! What it gives up is reproducibility: a shipped row was taken on an idle machine and
//! is pinned by CI, and this one is taken on whatever the caller's process is doing at
//! the time. The mitigation is the same one the mint uses — a minimum over several
//! traversals, and every ratio's legs [interleaved](Bench::rounds) so contention falls
//! on numerator and denominator alike.
//!
//! # What it refuses to do
//!
//! Every refusal in [`Unmeasurable`] is a case where a row *could* have been returned
//! and would have been fiction. A sample too small to time is the important one: at a
//! few kilobytes the timer's own resolution is the measurement, and the resulting row
//! would arm patterns on noise. This module would rather hand back an error than a
//! plausible-looking [`Calibration`], for exactly the reason
//! [`UNMEASURED`](super::UNMEASURED) exists.

use alloc::vec::Vec;
use std::time::Instant;

use regex_automata::Input;
use regex_automata::dfa::{Automaton, dense};
use regex_automata::nfa::thompson;
use regex_automata::util::syntax;

use super::{Calibration, REGIMES, Residency};
use crate::arch::{ARCH, OS};
use crate::lattice::MAX_CONJUNCTS;
use crate::{Gate, Policy, Sieve, shuffle};

/// The corpus volume below which nothing here can be timed honestly.
///
/// One traversal of this many bytes at the crate's own coefficients takes tens of
/// microseconds, which is four orders of magnitude above the resolution of any
/// [`Instant`] a supported target provides. Below it the ratio being measured is the
/// clock's granularity, and the row would be noise wearing six decimal places.
///
/// Deliberately far under [`RESIDENT_ABOVE`](super::RESIDENT_ABOVE): a caller who hands
/// this much gets an honest **cache-resident** row, which is a real regime and often the
/// one they are actually in. What a small sample cannot produce is the memory-resident
/// column, and [`Bench::measure`] does not pretend otherwise — it fills the column its
/// sample size earns and leaves the other reading unmeasured.
pub const MEASURABLE_ABOVE: usize = 64 << 10;

/// How many traversals a coefficient is the minimum of, unless [`Bench::rounds`] says
/// otherwise.
///
/// The minimum is what discards the cold first pass, so this is the number of chances a
/// loop gets to run without being descheduled. Seven is what the mint has always used.
pub const ROUNDS: usize = 7;

/// Why no row could be taken.
///
/// Every variant is a case where returning a [`Calibration`] anyway would have been
/// fiction — see the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unmeasurable {
    /// The sample is too small for the clock. Hand [`MEASURABLE_ABOVE`] bytes or more.
    TooFewBytes {
        /// What the sample held.
        bytes: usize,
        /// [`MEASURABLE_ABOVE`].
        floor: usize,
    },
    /// The sample contains the byte sequence the reference patterns are built *not* to
    /// find, so those patterns would match and every timing would be of an early exit
    /// rather than of a full traversal.
    ///
    /// `\x00\x01zz` is the sentinel, chosen because text does not contain it — a sample
    /// that holds those four bytes *raw* is binary. Note that source text naming the
    /// sequence in escaped form, as this file does, holds no such bytes and is fine:
    /// what is searched for is the sequence, not its spelling.
    ///
    /// The fix is to sample the documents actually being searched, or to drop the ones
    /// holding it. What is not on offer is timing against them anyway, since every
    /// coefficient would then be a measurement of how quickly the engine gives up.
    ProbeMatched,
    /// The clock reported that a full traversal took no time at all, so no per-byte
    /// figure can be derived from it. A sample above [`MEASURABLE_ABOVE`] on a machine
    /// with a working monotonic clock does not reach this.
    NoElapsedTime,
}

impl core::fmt::Display for Unmeasurable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooFewBytes { bytes, floor } => write!(
                f,
                "cannot price this machine from {bytes} bytes: under {floor}, the timer's \
                 own resolution is the measurement"
            ),
            Self::ProbeMatched => write!(
                f,
                "the sample holds the \\x00\\x01zz sentinel, so the reference patterns \
                 would match and time an early exit rather than a traversal"
            ),
            Self::NoElapsedTime => {
                write!(f, "a full traversal of the sample took no measurable time")
            },
        }
    }
}

impl core::error::Error for Unmeasurable {}

/// The sentinel every reference pattern requires. Absent from the sample, none of them
/// can match — so one search for it validates the whole slate at once.
const SENTINEL: &[u8] = b"\x00\x01zz";

/// One rare byte leaves an escape set of one, which the engine accelerates: this times
/// `memchr` throughput.
const SKIP_REF: &str = r"(?-u)\x00\x01zz";
/// A 52-byte class is far over the engine's accelerator threshold, so it walks: this
/// times the dependent-load DFA.
const WALK_REF: &str = r"(?-u)[A-Za-z]\x00\x01zz";

/// Lead bytes the engine's excursion coefficient is solved over, each beside the
/// pattern that leads with it.
///
/// Eleven of them, spanning four character classes and two orders of magnitude of
/// frequency, because a coefficient fitted to one letter is a fit rather than a
/// measurement. The byte is carried beside the pattern rather than derived from it so
/// the escape frequency and the thing timed cannot drift apart.
const LEADS: &[(u8, &str)] = &[
    (b'e', r"(?-u)e\x00\x01zz"),
    (b't', r"(?-u)t\x00\x01zz"),
    (b'a', r"(?-u)a\x00\x01zz"),
    (b'o', r"(?-u)o\x00\x01zz"),
    (b's', r"(?-u)s\x00\x01zz"),
    (b'f', r"(?-u)f\x00\x01zz"),
    (b'p', r"(?-u)p\x00\x01zz"),
    (b'E', r"(?-u)E\x00\x01zz"),
    (b'3', r"(?-u)3\x00\x01zz"),
    (b'=', r"(?-u)=\x00\x01zz"),
    (b'.', r"(?-u)\.\x00\x01zz"),
];

/// Patterns whose quotient start block escapes on a **narrow** set, exercising
/// [`Instrument::Few`](crate::Instrument).
const FEW: &[&str] = &[
    r"(?-u)e\x00\x01zz",
    r"(?-u)a\x00\x01zz",
    r"(?-u)p\x00\x01zz",
    r"(?-u)E\x00\x01zz",
    r"(?-u)(alpha|beta|gamma)\x00\x01zz",
];

/// The same for a **wide** escape set, exercising the nibble classifier.
const WIDE: &[&str] = &[
    r"(?-u)[0-9]\x00\x01zz",
    r"(?-u)[A-Z]\x00\x01zz",
    r"(?-u)[aeiou]\x00\x01zz",
    r"(?-u)[0-9a-fA-F]\x00\x01zz",
    r"(?-u)[.,;:(){}]\x00\x01zz",
];

/// Patterns spanning the conjunct counts the sieve coefficient is indexed by. A count
/// nothing here harvests stays zero, which [`Calibration::sieve_per_byte`] reads as
/// infinity — never as free.
const SLATE: &[&str] = &[
    r"(?-u)WalletService",
    r"(?-u)a[^\n]*b",
    r"(?-u)(alpha|beta|gamma)",
    r"(?-u)[A-Z][a-z]+Service",
    r"(?-u)[0-9]{3}-[0-9]{4}",
    r"(?-u)<[^>]*>",
    r"(?-u)ab+c",
];

/// The lowest an excursion multiplier can physically be: an escape byte costs at least
/// the byte itself.
///
/// What an unsolvable [`Calibration::dfa_excursion`] falls back to, and the direction is
/// chosen rather than convenient. That coefficient prices the **rival**, so erring high
/// there would flatter the sieve — the pessimistic fallback is the low one. Its sibling
/// [`Calibration::skip_excursion`] prices the *sieve*, so it errs the other way and
/// inherits the engine's figure instead.
const FLOOR_EXCURSION: f64 = 1.0;

/// One solved excursion coefficient, kept with the evidence behind it.
///
/// Published rather than folded away because the *spread* across a slate is the honest
/// measure of how much a single averaged coefficient can carry, and a mean with no
/// spread beside it is the kind of number that looks like a measurement and is not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Solution {
    /// The pattern that was timed.
    pub pattern: &'static str,
    /// The share of the sample its escape set covers, under the sample's own marginals.
    pub escape: f64,
    /// What it measured, in nanoseconds per byte.
    pub ns: f64,
    /// The excursion multiplier that inverts the blend for this pattern.
    pub excursion: f64,
}

/// Which conjunct count each slate pattern harvested — the census that says which slots
/// of [`Calibration::sieve`] a sample could speak for at all.
pub type Census = Vec<(&'static str, usize)>;

/// A [`Calibration`] and everything that went into it.
///
/// [`Bench::measure`] hands back the row alone, which is what a caller arming a sieve
/// wants. This is for the caller *publishing* a row — `examples/mint.rs` — who has to
/// print the per-pattern solutions and their spread so a human deciding whether to paste
/// the row can see how well-determined it is.
#[derive(Debug, Clone)]
pub struct Report {
    /// The row. Its measured column is [`Report::at`].
    pub calibration: Calibration,
    /// Which regime the sample put the timing loops in — see [`Bench::regime`].
    pub at: Residency,
    /// How many bytes were swept, per round.
    pub bytes: usize,
    /// Every independent solution of the engine's own excursion coefficient.
    pub engine: Vec<Solution>,
    /// The same per [`Instrument`](crate::Instrument), for the sieve's skip loop. An
    /// empty vector means the slate could not reach that instrument, and the row
    /// inherited the engine's coefficient there rather than a guess.
    pub probes: [Vec<Solution>; 2],
    /// See [`Census`].
    pub conjuncts: Census,
}

impl Report {
    /// Mean, lowest and highest of a set of solutions, or `None` for an empty one.
    ///
    /// The three numbers a reader needs together: a mean whose range is tight is a
    /// coefficient, and one whose range spans an order of magnitude is a slate
    /// disagreeing about what was measured.
    #[must_use]
    pub fn spread(solutions: &[Solution]) -> Option<(f64, f64, f64)> {
        if solutions.is_empty() {
            return None;
        }
        let each = || solutions.iter().map(|s| s.excursion);
        let mean = each().sum::<f64>() / solutions.len() as f64;
        Some((
            mean,
            each().fold(f64::MAX, f64::min),
            each().fold(0.0f64, f64::max),
        ))
    }
}

/// A measurement in progress: the caller's bytes, and how many chances each loop gets.
///
/// Built by [`Bench::new`] and consumed by [`Bench::measure`].
/// [`Calibration::measure`] is this with the defaults, which is what almost every caller
/// wants.
#[derive(Debug, Clone, Copy)]
pub struct Bench<'a> {
    docs: &'a [&'a [u8]],
    rounds: usize,
}

impl<'a> Bench<'a> {
    /// Price this machine over `docs` — a sample of the documents the caller is really
    /// going to search.
    ///
    /// Real bytes, and it matters twice over. The byte marginals every escape set is
    /// priced under are computed from exactly these bytes, so the row is keyed to this
    /// corpus rather than to one of the four shipped priors; and the residency column it
    /// fills is decided by how many bytes there are, because that is what physically
    /// determines whether the timing loop read cache or memory.
    ///
    /// Synthetic filler defeats both. A megabyte of `b'a'` reports a byte distribution
    /// no engine will ever meet, over a memory system every prefetcher can predict.
    #[must_use]
    pub const fn new(docs: &'a [&'a [u8]]) -> Self {
        Self {
            docs,
            rounds: ROUNDS,
        }
    }

    /// How many traversals each coefficient is the minimum over. Default [`ROUNDS`].
    ///
    /// More rounds buy a better chance that some pass ran uninterrupted, at linear cost
    /// in wall time. Fewer are for a caller pricing a very large sample who would rather
    /// spend the budget on bytes — the better trade, since a larger sample improves the
    /// marginals as well as the timing. Zero is read as one.
    #[must_use]
    pub const fn rounds(mut self, rounds: usize) -> Self {
        self.rounds = if rounds == 0 { 1 } else { rounds };
        self
    }

    /// Total bytes in the sample.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.docs.iter().map(|doc| doc.len()).sum()
    }

    /// Which regime a traversal of this sample runs in, and therefore which column
    /// [`Bench::measure`] can honestly fill.
    ///
    /// Inferred rather than accepted from the caller, and this is the one place in the
    /// crate where that is the right way round. Everywhere else a residency is the
    /// caller's fact about their own scan, which nothing here can probe. Here it is a
    /// fact about *the timing loop that is about to run*, whose corpus is in hand — so an
    /// override would only let a caller mislabel the column they just measured.
    #[must_use]
    pub fn regime(&self) -> Residency {
        Residency::of_working_set(self.bytes())
    }

    /// Take the row.
    ///
    /// Fills the column for [`Bench::regime`] and leaves the other zero, because one
    /// sample is one memory regime and a row claiming both from one sweep would be two
    /// claims from one measurement. [`Calibration::is_measured`] reports the unfilled
    /// column as unmeasured, so a caller who wants both prices a small sample and a large
    /// one and [`Calibration::merge`]s them.
    ///
    /// Costs roughly `rounds * 30` traversals of the sample. On a megabyte that is
    /// milliseconds; on the 64 MiB a memory-resident column really wants, seconds.
    pub fn measure(&self) -> Result<Calibration, Unmeasurable> {
        self.report().map(|report| report.calibration)
    }

    /// [`Bench::measure`] with the evidence attached — see [`Report`].
    pub fn report(&self) -> Result<Report, Unmeasurable> {
        let bytes = self.bytes();
        if bytes < MEASURABLE_ABOVE {
            return Err(Unmeasurable::TooFewBytes {
                bytes,
                floor: MEASURABLE_ABOVE,
            });
        }
        // Checked once for the whole slate rather than asserted per pattern: every
        // reference pattern below requires this literal, so its absence is the single
        // precondition that makes all of them time a full traversal.
        if self
            .docs
            .iter()
            .any(|doc| memchr::memmem::find(doc, SENTINEL).is_some())
        {
            return Err(Unmeasurable::ProbeMatched);
        }

        let freq = histogram(self.docs);
        let at = self.regime();
        let regime = at as usize;

        let walk = self.per_byte(&mut searcher(WALK_REF))?;
        let skip = self.per_byte(&mut searcher(SKIP_REF))?;

        let engine = self.excursion(&freq);
        // The mean, where the sieve's own coefficient below takes the maximum. This one
        // prices the rival, so erring high here would flatter the sieve.
        let solved = Report::spread(&engine).map_or(FLOOR_EXCURSION, |(mean, ..)| mean);
        let probes = self.probes(&freq);

        let mut dfa_skip = [0.0; REGIMES];
        let mut dfa_excursion = [0.0; REGIMES];
        let mut skip_excursion = [[0.0; REGIMES]; 2];
        dfa_skip[regime] = skip;
        dfa_excursion[regime] = solved.max(FLOOR_EXCURSION);
        for (slot, each) in probes.iter().enumerate() {
            // The **worst** solution, and an instrument the slate could not reach
            // inherits the engine's coefficient rather than a guess. Both are the same
            // choice: this term prices the sieve, so erring high can only decline a skip
            // — and it is what covers what the model structurally cannot see, since
            // excursion *length* is a property of the quotient rather than of the escape
            // frequency.
            skip_excursion[slot][regime] = Report::spread(each).map_or(solved, |(.., hi)| hi);
        }

        let (sieve, conjuncts) = self.sieve()?;
        Ok(Report {
            calibration: Calibration {
                os: OS,
                arch: ARCH,
                kernel: shuffle::kernel(),
                // Not a machine description, and cannot be one: a `&'static str` cannot
                // hold a string built at run time, and the fields a row is *resolved* by
                // are the three above. These two are provenance for a human reading a
                // `Debug`, and the honest provenance of this row is that it was taken
                // here rather than shipped.
                host: "measured at run time",
                minted: "runtime",
                dfa_skip,
                dfa_walk: walk,
                dfa_excursion,
                skip_excursion,
                sieve,
            },
            at,
            bytes,
            engine,
            probes,
            conjuncts,
        })
    }

    /// The engine's excursion multiplier, solved rather than assumed.
    ///
    /// Time an accelerated pattern whose lead byte trips the skip, then invert the blend
    /// `measured = skip*(1-p) + walk*p*E` for `E`, with the escape frequency `p` read
    /// from the same per-byte table the gate will use — so the solver and the model
    /// cannot disagree about what `p` means.
    ///
    /// Each solution's two baselines are re-timed [`paired`](Bench::paired) with the
    /// pattern they normalize, because a ratio is only a measurement when numerator and
    /// denominator saw the same machine.
    fn excursion(&self, freq: &[f64; 256]) -> Vec<Solution> {
        let mut solved = Vec::new();
        for &(lead, pattern) in LEADS {
            let escape = freq[usize::from(lead)];
            let Ok([ns, skip, walk]) = self.triple(pattern) else {
                continue;
            };
            let excursion = (ns - skip * (1.0 - escape)) / (walk * escape);
            if excursion.is_finite() && excursion > 0.0 {
                solved.push(Solution {
                    pattern,
                    escape,
                    ns,
                    excursion,
                });
            }
        }
        solved
    }

    /// The same inversion for the sieve's own [`Skip`](crate::Skip) loop, once per
    /// instrument.
    ///
    /// A separate number because it is a separate physical event — the engine's excursion
    /// re-enters a dense DFA that does not fit in L1, the sieve's re-enters sixteen
    /// blocks that do.
    fn probes(&self, freq: &[f64; 256]) -> [Vec<Solution>; 2] {
        let mut solved = [Vec::new(), Vec::new()];
        for (slot, slate) in [FEW, WIDE].iter().enumerate() {
            for &pattern in *slate {
                let Some((quotient, probe)) = harvest_skip(pattern) else {
                    continue;
                };
                if probe.instrument() as usize != slot {
                    continue;
                }
                let escape: f64 = probe
                    .leaves()
                    .iter()
                    .map(|&b| freq[usize::from(b)])
                    .sum::<f64>()
                    .clamp(0.0, 1.0);
                let Ok([ns, skip, walk]) = self.paired(&mut [
                    &mut |hay: &[u8]| {
                        core::hint::black_box(shuffle::refutes_skipping(&quotient, &probe, hay));
                    },
                    &mut searcher(SKIP_REF),
                    &mut searcher(WALK_REF),
                ]) else {
                    continue;
                };
                let excursion = (ns - skip * (1.0 - escape)) / (walk * escape);
                if excursion.is_finite() && excursion > 0.0 {
                    solved[slot].push(Solution {
                        pattern,
                        escape,
                        ns,
                        excursion,
                    });
                }
            }
        }
        solved
    }

    /// The composition kernel's price at each conjunct count, beside the census of which
    /// count each slate pattern reached.
    ///
    /// Built with `skip: false` throughout, and that is not a detail: this coefficient is
    /// the number a candidate skip is compared against, so letting these timings take the
    /// skip path would be setting the exchange rate in the currency being measured.
    fn sieve(&self) -> Result<([f64; MAX_CONJUNCTS], Census), Unmeasurable> {
        // `Gate::Ungated` consults no price, which is the only way to build on a machine
        // that has no row yet — the circularity this whole module exists to break.
        let composing = Policy {
            gate: Gate::Ungated,
            skip: false,
            ..Policy::new(Residency::Memory)
        };
        let built: Vec<(&'static str, Sieve)> = SLATE
            .iter()
            .filter_map(|&p| Sieve::with(p, &composing).ok().map(|s| (p, s)))
            .collect();
        let census = built
            .iter()
            .map(|(pattern, sieve)| (*pattern, sieve.conjuncts()))
            .collect();
        let mut sieve = [0.0; MAX_CONJUNCTS];
        for (n, slot) in sieve.iter_mut().enumerate() {
            let Some((_, one)) = built.iter().find(|(_, s)| s.conjuncts() == n + 1) else {
                continue;
            };
            *slot = self.per_byte(&mut |hay: &[u8]| {
                core::hint::black_box(one.refutes(hay));
            })?;
        }
        Ok((sieve, census))
    }

    /// One pattern against both baselines, interleaved.
    fn triple(&self, pattern: &str) -> Result<[f64; 3], Unmeasurable> {
        self.paired(&mut [
            &mut searcher(pattern),
            &mut searcher(SKIP_REF),
            &mut searcher(WALK_REF),
        ])
    }

    /// Time three loops **interleaved** — one traversal of each per round — and hand back
    /// each one's own minimum.
    ///
    /// Interleaved because a ratio is only a measurement when its numerator and
    /// denominator saw the same machine, and contention does not fall equally on every
    /// loop: a branchy excursion degrades further under load than a streaming `memchr`
    /// does. Inverting an excursion timed now against a baseline timed a minute ago
    /// measures the drift between the two moments rather than the excursion.
    fn paired(&self, runs: &mut [Leg<'_>; 3]) -> Result<[f64; 3], Unmeasurable> {
        let bytes = self.bytes();
        let mut best = [f64::MAX; 3];
        for _ in 0..self.rounds {
            for (slot, run) in runs.iter_mut().enumerate() {
                let started = Instant::now();
                for doc in self.docs {
                    run(doc);
                }
                best[slot] = best[slot].min(started.elapsed().as_secs_f64());
            }
        }
        if best.iter().any(|&secs| secs <= 0.0 || !secs.is_finite()) {
            return Err(Unmeasurable::NoElapsedTime);
        }
        Ok(best.map(|secs| secs * 1e9 / bytes as f64))
    }

    /// Nanoseconds per byte for one loop, as the minimum over [`Bench::rounds`]
    /// traversals.
    ///
    /// A minimum rather than a mean, and it is what makes the two residency columns need
    /// no separate machinery. Over a sample that fits in cache every pass after the first
    /// is warm, so the minimum is a cache-resident measurement; over one that does not, no
    /// pass is ever warm, so the same minimum is a memory-resident one. Same timer, same
    /// loop — the sample size is the independent variable.
    fn per_byte(&self, run: &mut dyn FnMut(&[u8])) -> Result<f64, Unmeasurable> {
        let bytes = self.bytes();
        let mut best = f64::MAX;
        for _ in 0..self.rounds {
            let started = Instant::now();
            for doc in self.docs {
                run(doc);
            }
            best = best.min(started.elapsed().as_secs_f64());
        }
        if best <= 0.0 || !best.is_finite() {
            return Err(Unmeasurable::NoElapsedTime);
        }
        Ok(best * 1e9 / bytes as f64)
    }
}

/// One leg of a paired measurement: a loop to sweep the sample with.
type Leg<'a> = &'a mut dyn FnMut(&[u8]);

impl Calibration {
    /// Price the running machine over `docs`, with the default [`ROUNDS`].
    ///
    /// The one-line front door to [`Bench`], and what a caller on an unminted
    /// architecture reaches for: hand it a sample of the documents this process searches,
    /// put the result in [`Policy::calibration`](crate::Policy::calibration), and the gate
    /// starts making decisions about *this* machine instead of declining for want of a
    /// row.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let docs: Vec<&[u8]> = Vec::new();
    /// use sheng::price::Calibration;
    ///
    /// let mine = Calibration::measure(&docs)?;
    /// assert_eq!(mine.regime().map(|at| mine.is_measured(at)), Some(true));
    /// # Ok(()) }
    /// ```
    pub fn measure(docs: &[&[u8]]) -> Result<Self, Unmeasurable> {
        Bench::new(docs).measure()
    }

    /// The one regime this row prices, or `None` if it prices none or both.
    ///
    /// What a [`Bench`] leaves behind, and the field a caller needs in order to state a
    /// [`Policy`](crate::Policy) this row can actually answer: a measured row describes
    /// the memory regime of the sample it was taken over, and asking it about the other
    /// one gets [`BuildError::Uncalibrated`](crate::BuildError::Uncalibrated).
    ///
    /// `None` in both directions, since neither is a single answer — a shipped row that
    /// measured both regimes is not describing one, and
    /// [`UNMEASURED`](super::UNMEASURED) describes nothing at all.
    #[must_use]
    pub fn regime(&self) -> Option<Residency> {
        let mut measured = Residency::ALL
            .into_iter()
            .filter(|&at| self.is_measured(at));
        measured.next().filter(|_| measured.next().is_none())
    }

    /// Combine two rows for the same machine, taking each regime column from whichever
    /// row measured it.
    ///
    /// For the caller — and for `examples/mint.rs` — who prices a cache-sized sample and
    /// a memory-sized one and wants the single row a shipped [`MINTED`](super::MINTED)
    /// entry would be. `self` wins any column both measured.
    ///
    /// # When it refuses
    ///
    /// `None` unless the two rows describe the same `(os, arch, kernel)` triple, which is
    /// the whole of what makes combining them legitimate. And `None` when the resulting
    /// cache-resident column would be **more expensive** than the memory-resident one:
    /// a hotter haystack cannot cost the engine more, so a pair that came out that way
    /// measured a busy machine rather than a memory system. This is the only moment at
    /// which that is detectable, since within one sweep there is a single column to look
    /// at — which is why it is a refusal here rather than a warning anywhere.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Option<Self> {
        if (self.os, self.arch, self.kernel) != (other.os, other.arch, other.kernel) {
            return None;
        }
        let mut merged = *self;
        for at in Residency::ALL {
            if self.is_measured(at) {
                continue;
            }
            let i = at as usize;
            merged.dfa_skip[i] = other.dfa_skip[i];
            merged.dfa_excursion[i] = other.dfa_excursion[i];
            for (slot, instrument) in merged.skip_excursion.iter_mut().enumerate() {
                instrument[i] = other.skip_excursion[slot][i];
            }
        }
        // `dfa_walk` and `sieve` carry no regime, so they are taken from whichever row
        // holds them rather than per column — and from `other` only where `self` has
        // nothing, which is what "self wins any column both measured" means for the two
        // coefficients that are not columns.
        if merged.dfa_walk <= 0.0 {
            merged.dfa_walk = other.dfa_walk;
        }
        if !merged.sieve.iter().any(|&cost| cost > 0.0) {
            merged.sieve = other.sieve;
        }
        let priced = |at: Residency| merged.is_measured(at);
        let hot = Residency::Cache as usize;
        let cold = Residency::Memory as usize;
        if priced(Residency::Cache)
            && priced(Residency::Memory)
            && merged.dfa_skip[hot] > merged.dfa_skip[cold]
        {
            return None;
        }
        Some(merged)
    }
}

/// Marginal frequency of every byte value in the sample.
///
/// **Per-byte** rather than per-class, because that is the resolution the escape-set
/// model needs: within `Lower`, `a` is several times commoner than `f`, and pricing them
/// alike is the difference between arming a clear winner and arming a clear loser.
#[must_use]
pub fn histogram(docs: &[&[u8]]) -> [f64; 256] {
    let mut n = [0u64; 256];
    for doc in docs {
        for &b in *doc {
            n[usize::from(b)] += 1;
        }
    }
    let total: u64 = n.iter().sum();
    n.map(|count| {
        if total == 0 {
            0.0
        } else {
            count as f64 / total as f64
        }
    })
}

fn matcher(pattern: &str) -> Option<dense::DFA<Vec<u32>>> {
    dense::Builder::new()
        .syntax(syntax::Config::new().utf8(false))
        .thompson(thompson::Config::new().utf8(false))
        .build(pattern)
        .ok()
}

/// A closure that runs one full engine search, for use as a timed leg.
///
/// A pattern that will not build sweeps nothing, which costs one solution rather than
/// the whole measurement. The reference patterns are constants here, so that is
/// unreachable in practice — and it is a refusal rather than a panic because a library
/// measuring a caller's bytes has no business aborting their process.
fn searcher(pattern: &str) -> impl FnMut(&[u8]) {
    let dfa = matcher(pattern);
    move |hay: &[u8]| {
        if let Some(dfa) = &dfa {
            core::hint::black_box(dfa.try_search_fwd(&Input::new(hay)).ok());
        }
    }
}

/// The first harvested quotient for `pattern` and the skip over its start block, or
/// `None` when the pattern yields neither.
fn harvest_skip(pattern: &str) -> Option<(crate::Quotient, crate::Skip)> {
    let dfa = matcher(pattern)?;
    let core = crate::Projection::of(&dfa).ok()?;
    let quotient = crate::harvest(&core).into_iter().next()?;
    let probe = crate::Skip::of(&quotient.rows, quotient.start)?;
    (quotient.start < quotient.threshold).then_some((quotient, probe))
}
