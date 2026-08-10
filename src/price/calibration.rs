//! The `Calibration` shape and the per-byte arithmetic it answers — resolving the
//! running machine's row is [`active`]; the rows themselves live in
//! [`super::minted`].

use super::minted::{MINTED, UNMEASURED};
use crate::arch::{ARCH, OS};
use crate::lattice::MAX_CONJUNCTS;
use crate::shuffle::{self, Kernel};
use crate::skip::Skip;

/// Where the bytes a caller is about to search are coming from.
///
/// A per-byte price is only a price against a particular memory system, and this is
/// the one fact about the caller's scan that no amount of arithmetic can recover from
/// the pattern. It has no default: see [`crate::Policy::new`].
///
/// # Why this had to become a dimension
///
/// [`Calibration::rival_per_byte`] caps the engine's price at [`Calibration::dfa_walk`],
/// and that cap is what decides whether a pattern is exposed to this at all. A rival
/// with a *frequent* escape set is pinned at the cap — a dependent-load walk, bound by
/// L1 latency and indifferent to where the haystack lives — so the sieve's advantage
/// over it holds in every regime. A rival with a *rare* escape set instead rides
/// [`Calibration::dfa_skip`], which in the memory-resident regime is pinned by DRAM
/// bandwidth and in the cache-resident regime is not pinned by anything the mint
/// measured.
///
/// The gap is not academic. `panic!\(` can price as a clear win over a large
/// memory-resident corpus and measure as a clear loss over a cache-resident one —
/// same pattern, same machine, same coefficients. The engine's accelerated path
/// moves with residency because its excursion re-enters a dense DFA whose
/// transition table misses cache in one regime and hits it in the other. The
/// sieve's own cost barely moves, which is exactly why a single row could not
/// describe both: the comparison is between a term that moves and a term that
/// does not.
///
/// So the uncomfortable half of the finding, stated where a caller will read it: the
/// sieve's edge over an accelerated engine comes substantially from *that engine
/// missing cache*. Remove the memory pressure and the edge shrinks rather than merely
/// scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Residency {
    /// The bytes are already in cache when the scan reaches them — a working set that
    /// fits in last-level cache, or one scanned repeatedly. The regime in which the
    /// engine's `memchr` is at its cheapest and the sieve therefore at its least
    /// competitive.
    Cache = 0,
    /// The bytes are being read from main memory: a corpus larger than last-level
    /// cache, traversed once. The regime the shipped rows were originally minted over.
    Memory = 1,
}

/// How many regimes a regime-indexed coefficient carries. Not a tuning knob — it is
/// the variant count of [`Residency`], and the array indexing below depends on it.
pub const REGIMES: usize = 2;

/// The volume above which one pass over a corpus is reading from main memory rather
/// than from cache.
///
/// Eight mebibytes is past the last-level cache of every machine [`MINTED`] names,
/// which is the whole of what this constant claims. It is a number rather than a probe
/// because cache geometry is not portably knowable — reading it means an operating
/// system call this crate does not make on any target, and would still not answer the
/// question, since what matters is how much of that cache the caller's own working set
/// is competing for.
pub const RESIDENT_ABOVE: usize = 8 << 20;

impl Residency {
    /// Every regime, in coefficient-index order — so a mint can emit each column and a
    /// test can sweep them without either restating the variant list and drifting.
    pub const ALL: [Self; REGIMES] = [Self::Cache, Self::Memory];

    /// Which regime a single pass over `bytes` of haystack runs in.
    ///
    /// [`crate::Policy::new`] asks for a residency rather than guessing one because this
    /// crate never sees the corpus. That reasoning does not extend to the caller, who
    /// usually knows exactly how many bytes they are about to hand the engine — and for
    /// them the answer is arithmetic against [`RESIDENT_ABOVE`] rather than a judgement
    /// call about somebody's cache hierarchy. `examples/survey.rs` is this call, and it
    /// used to be this constant copied into the example.
    ///
    /// # Which way it is wrong, and who has to override it
    ///
    /// Two situations make a corpus cache-resident that this cannot see, and both read
    /// as [`Residency::Memory`] here: a working set **re-scanned** rather than traversed
    /// once is resident however large it is, and a machine whose last-level cache
    /// exceeds [`RESIDENT_ABOVE`] holds more than this admits.
    ///
    /// That is the unsafe direction, and it is named here rather than buried because it
    /// is the one error this whole helper can make. `Memory` is the regime a sieve looks
    /// *better* in — the engine's `memchr` is cheapest exactly where the sieve is least
    /// competitive — so a caller in either situation who takes this answer arms patterns
    /// that then measure as losses. They should state [`Residency::Cache`] outright.
    /// A caller streaming a corpus once, which is what the count in hand usually means,
    /// is the case this answers correctly.
    #[must_use]
    pub const fn of_working_set(bytes: usize) -> Self {
        if bytes > RESIDENT_ABOVE {
            Self::Memory
        } else {
            Self::Cache
        }
    }
}

/// Per-byte costs for every kernel the gate weighs, each timed **alone** so one
/// coefficient can be re-minted without re-deriving any other.
///
/// # Which coefficients carry a regime, and which cannot
///
/// Two of these are indexed by [`Residency`] and two are not, and the split is a
/// claim about what each loop is bound by rather than a convenience:
///
/// * [`Calibration::dfa_skip`] and the excursion coefficients **are** regime-indexed.
///   A `memchr` stream is bandwidth-bound, and an excursion's dominant cost is
///   re-entering a transition table that may or may not be resident.
/// * [`Calibration::dfa_walk`] is **not**. A dependent-load DFA walk waits on L1
///   latency for a table it has already pulled in, one state at a time, and measures
///   nearly the same on both architectures — the same number in both regimes for
///   the same reason it is nearly the same number on both machines.
/// * [`Calibration::sieve`] is **not**. The composition kernel is issue-bound at three
///   operations a byte and runs an order of magnitude under the bandwidth a
///   `memchr` saturates, so it has no headroom to gain from a hotter haystack.
///
/// Keeping them in one row rather than shipping two rows per machine is deliberate.
/// A row is a claim about an (operating system, architecture, kernel) triple, and the
/// invariance above is
/// then a structural fact the type enforces instead of a coincidence two independently
/// pasted rows would have to be trusted to preserve.
#[derive(Debug, Clone, Copy)]
pub struct Calibration {
    /// The operating system these ratios were measured under, spelled as [`OS`] spells
    /// it so [`active`] can match it against the running target.
    ///
    /// Part of the key because a row is a claim about *one machine*, and (`os`, `arch`)
    /// is the finest description of a machine a running binary can ask about itself —
    /// see [`OS`] for the measurement that forced this column into existence.
    pub os: &'static str,
    /// The instruction set these ratios describe, spelled as [`ARCH`] spells it so
    /// [`active`] can match it against the running target.
    pub arch: &'static str,
    /// The kernel the `sieve` coefficients were timed with. A number measured through
    /// a byte shuffle is not a number for a target that has none, which is why this
    /// is matched and not assumed.
    pub kernel: Kernel,
    /// The silicon that produced these numbers, and when. A measured value with no
    /// machine beside it is an anecdote.
    pub host: &'static str,
    /// The date this row was measured, `YYYY-MM-DD`. What ages a row into stale.
    pub minted: &'static str,
    /// `regex-automata`'s dense DFA over bytes its start-state accelerator skips —
    /// effectively `memchr` throughput. Indexed by [`Residency`], because a byte
    /// scan this fast is bound by how quickly bytes arrive rather than by what it
    /// does with them: out of memory it saturates single-core DRAM bandwidth, which
    /// is not a property of the loop.
    pub dfa_skip: [f64; REGIMES],
    /// The same DFA with no accelerator to skip with: the dependent-load walk. Not
    /// regime-indexed — see the type's own documentation.
    pub dfa_walk: f64,
    /// Bytes charged at walk price per escape byte the accelerator trips over.
    ///
    /// An accelerated engine does not pay for one byte when `memchr` finds a
    /// candidate — it enters the DFA, walks a short run, returns, and restarts the
    /// skip, and the restart is most of the cost at this granularity. Without this
    /// term the model under-priced a common-byte accelerator by nearly an order of
    /// magnitude, which declined patterns that genuinely paid.
    ///
    /// Indexed by [`Residency`] because the re-entry is exactly where the memory
    /// system shows up: the table the excursion walks into is the engine's *dense*
    /// DFA, which is far too large for L1, so whether it is otherwise resident
    /// changes the escape cost by about a factor of two.
    pub dfa_excursion: [f64; REGIMES],
    /// The same quantity for the sieve's own [`crate::Skip`] loop, per instrument and
    /// per regime.
    ///
    /// A separate number because it is a separate physical event. The engine's
    /// excursion leaves `memchr`, enters a dense DFA whose table does not fit in
    /// L1, walks, and restarts the accelerator; the sieve's enters a sixteen-block
    /// quotient that does, and resumes a probe whose two tables are already in
    /// registers. Measured, the sieve's classifier excursion is a few times
    /// cheaper — and charging it the engine's rate declined skips that paid.
    ///
    /// The outer index is [`crate::skip::Instrument`], because the instruments restart
    /// at genuinely different prices: `memchr` re-enters an aligned multi-stage loop,
    /// the nibble classifier re-enters two registers and a sixteen-byte step. The
    /// inner index is [`Residency`], for the same reason `dfa_excursion` carries one —
    /// though the sieve's sixteen-byte table is resident in *either* regime, so this is
    /// the coefficient expected to move least.
    pub skip_excursion: [[f64; REGIMES]; 2],
    /// The sieve's own cost, indexed by conjunct count minus one. A zero means
    /// **never measured**, which [`Calibration::sieve_per_byte`] reports as
    /// infinity — a free
    /// pre-pass would pass every worth test.
    pub sieve: [f64; MAX_CONJUNCTS],
}

/// The calibration for the machine that is running, or [`UNMEASURED`].
///
/// Keyed on the operating system, the architecture, **and** the kernel that dispatch
/// actually chose. The kernel is in the key because an `x86_64` without SSSE3 runs the
/// scalar path and has no business inheriting a `pshufb` measurement; the operating
/// system is in it because a row describes one machine's memory system and a borrowed
/// row mis-arms — see [`OS`]. Resolved at run time rather than by `cfg` for the same
/// reason the kernel is: it is a runtime probe on x86, so a compile-time answer could be
/// wrong on the machine that ends up executing.
///
/// `residency` is **not** part of that key, and the asymmetry is the point. Which
/// silicon is running is a fact this crate can determine; which memory regime the
/// caller's scan is in is not, so it is asked for rather than probed. A row carries
/// both regimes ([`Calibration`]), so what this returns is the whole row and the regime
/// only selects which of its columns the gate reads — except that a row holding no
/// measurement for the regime asked about resolves to [`UNMEASURED`] rather than
/// answering out of the other one.
#[must_use]
pub fn active(residency: Residency) -> Calibration {
    let kernel = shuffle::kernel();
    MINTED
        .iter()
        .copied()
        .find(|cal| {
            cal.os == OS && cal.arch == ARCH && cal.kernel == kernel && cal.is_measured(residency)
        })
        .unwrap_or(UNMEASURED)
}

impl Calibration {
    /// Was anything here actually timed, *for the regime being asked about*?
    ///
    /// Regime-aware because a row can honestly hold one regime and not the other — a
    /// machine whose memory-resident coefficients were pasted in before its
    /// cache-resident ones were taken is a real state, and it has to read as
    /// uncalibrated for the regime it cannot price rather than borrow the one it can.
    /// That is the same refusal [`UNMEASURED`] makes about a whole machine, applied one
    /// column in.
    #[must_use]
    pub fn is_measured(&self, residency: Residency) -> bool {
        self.sieve.iter().any(|&cost| cost > 0.0)
            && self.dfa_walk > 0.0
            && self.dfa_skip[residency as usize] > 0.0
    }

    /// The sieve's per-byte price at `conjuncts`.
    ///
    /// A count nobody minted is extrapolated from the nearest one that was, and the
    /// direction decides how — both ways erring high, so an unmeasured slot can only
    /// make a sieve decline:
    ///
    /// * **Upward** (want more conjuncts than were measured): double per step. Each
    ///   conjunct is an independent pass over the same bytes, so twice the cost of
    ///   `n` is a sound ceiling for `n+1`.
    /// * **Downward** (want fewer): take the measurement unchanged. Fewer passes
    ///   cannot cost more than more passes, so a higher count's price is already an
    ///   upper bound — and pricing it lower would credit the short-circuit that
    ///   [`crate::Sieve::refutes`] only sometimes gets.
    ///
    /// With nothing measured at all the answer is infinity, never zero: a free
    /// pre-pass passes every worth test.
    #[must_use]
    pub fn sieve_per_byte(&self, conjuncts: usize) -> f64 {
        let want = conjuncts.clamp(1, MAX_CONJUNCTS) - 1;
        if let Some(below) = (0..=want).rev().find(|&i| self.sieve[i] > 0.0) {
            // One doubling per unmeasured step up. A power of two is exact in binary,
            // so a shift is the whole exponentiation.
            return self.sieve[below] * (1u64 << (want - below)) as f64;
        }
        self.sieve[want..]
            .iter()
            .copied()
            .find(|&cost| cost > 0.0)
            .unwrap_or(f64::INFINITY)
    }

    /// What the confirming engine costs per byte, given the bytes it told us it will
    /// skip.
    ///
    /// `accelerator` is `Automaton::accelerator` for the engine's start state: empty
    /// when the engine has no skip and is committed to a walk.
    ///
    /// Otherwise the engine skips most bytes and pays an excursion for each escape
    /// byte the prior expects, which is what makes a **rare** lead byte unbeatable
    /// and a **common** one barely an advantage at all. The result is capped at the
    /// unaccelerated walk: an accelerator that trips on everything degenerates to
    /// walking, never to something slower.
    ///
    /// That cap is also what decides whether this pattern is exposed to `residency` at
    /// all. A frequent escape set pins the answer at [`Calibration::dfa_walk`], which
    /// carries no regime; a rare one rides [`Calibration::dfa_skip`], which carries
    /// the whole of it. See [`Residency`].
    #[must_use]
    pub fn rival_per_byte(&self, accelerator: &[u8], freq: &[f64; 256], at: Residency) -> f64 {
        if accelerator.is_empty() {
            return self.dfa_walk;
        }
        let escape = share(accelerator, freq);
        let cost = self.dfa_skip[at as usize] * (1.0 - escape)
            + self.dfa_walk * escape * self.dfa_excursion[at as usize];
        cost.min(self.dfa_walk)
    }

    /// What the sieve's own [`crate::Skip`] loop costs per byte on this machine.
    ///
    /// The same blend as [`Calibration::rival_per_byte`] and for the same reason —
    /// a skip loop *is* an accelerated DFA, so there is one shape of arithmetic here,
    /// not two. What differs is the excursion coefficient, which is the instrument's
    /// own rather than the engine's.
    ///
    /// A block nothing leaves is the case the blend cannot state: the loop returns
    /// without reading a byte, so it is charged the cheapest coefficient the machine
    /// has rather than nothing at all.
    #[must_use]
    pub fn skip_per_byte(&self, skip: &Skip, freq: &[f64; 256], at: Residency) -> f64 {
        let leaves = skip.leaves();
        if leaves.is_empty() {
            return self.dfa_skip[at as usize];
        }
        let escape = share(leaves, freq);
        let excursion = self.skip_excursion[skip.instrument() as usize][at as usize];
        let cost = self.dfa_skip[at as usize] * (1.0 - escape) + self.dfa_walk * escape * excursion;
        cost.min(self.dfa_walk)
    }
}

/// What one confirming pass costs, and therefore what a refutation is worth.
///
/// [`Rival::Engine`] is the shipped answer and the right one whenever the work a
/// refutation skips really is a regex search: the price is read from the automaton's own
/// accelerator rather than assumed ([`Calibration::rival_per_byte`]).
///
/// # Why the other two exist
///
/// A sieve does not produce a faster scan. It produces a **proof that a document needs
/// no further work**, and what that proof is worth is decided entirely by what the
/// further work would have been. Against `regex-automata` it is worth very little,
/// because `regex-automata` is extremely fast — which is why the gate correctly declines
/// most patterns, and why a per-byte filter can never front a rare lead byte profitably
/// however selective it is.
///
/// That was unreachable while the rival's price could only be read off a [`crate::Dfa`],
/// because an automaton describes what the *pattern* costs to confirm and not what the
/// caller's pipeline costs to run. A caller could only forge a [`Calibration`] and misuse
/// [`Calibration::dfa_walk`] to mean something it does not, which would have corrupted
/// [`Calibration::skip_per_byte`] in the same motion. These two variants are where that
/// fact belongs.
///
/// # Which confirms are actually expensive
///
/// Worth stating in figures, because intuition is a poor guide here and the intuitive
/// answers are mostly wrong. [`Calibration::dfa_walk`] is between 1.3 and 2.1 ns per
/// byte on every machine [`MINTED`] names, so "expensive" means expensive against
/// roughly **two nanoseconds a byte** — which is a low bar for a network and a
/// surprisingly high one for anything running on the same core.
///
/// These do **not** qualify, and naming them is the more useful half: zstd or gzip
/// decompression lands around 1–3 ns/B, AES with hardware support under 1, and a decent
/// JSON parser 1–3. All are within a small multiple of a walk, so a sieve in front of
/// them is priced almost exactly as it is in front of the engine and should be, since
/// the gate's answer barely moves.
///
/// These do: extracting text from a PDF or an image, at hundreds of ns/B; an embedding
/// or any other model call, at thousands; a per-document network round trip, whose
/// latency alone dwarfs the scan; and full-text indexing with the writes it implies. For
/// these the rival term is two to four orders of magnitude above a walk, the gate is not
/// close, and selectivity is the only thing still deciding — which is the regime a
/// refutation sieve was always the right tool for.
///
/// # Which of the two to reach for
///
/// [`Rival::Walks`] is **dimensionless** — a multiple of this machine's own dense-DFA
/// walk — and is the one to prefer, because it is the one that keeps the promise this
/// module makes about clocks: scale every coefficient in a [`Calibration`] by any
/// positive constant and no decision moves. A ratio scales along with them. An absolute
/// duration does not, so mixing one into the inequality makes the verdict a function of
/// ambient load in exactly the way `scaling_the_whole_calibration_changes_no_decision`
/// exists to forbid.
///
/// [`Rival::NanosPerByte`] is for the caller who has *timed* their confirm and wants that
/// figure used as measured. It buys exactness about one machine and gives up the scale
/// invariance above. That trade is usually fine here and it is worth being precise about
/// why: this variant exists for confirms costing orders of magnitude more than a walk, and
/// no plausible clock drift moves a verdict decided by three orders of magnitude. It is a
/// bad choice near parity, which is the one place the invariance was ever load-bearing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rival {
    /// Ask the automaton — [`crate::Dfa::accelerator`] on its start state — and price the
    /// engine off what it says it will skip. The default, and the only variant that reads
    /// the automaton at all.
    Engine,
    /// A confirm costing this many dense-DFA walks per byte of document.
    ///
    /// Dimensionless, and therefore the variant that preserves scale invariance. A
    /// gzip-then-parse pipeline at roughly a hundred times a DFA walk is `Walks(100.0)`.
    Walks(f64),
    /// A confirm costing this many nanoseconds per byte of document, as measured.
    ///
    /// The unit every [`Calibration`] coefficient is already in, so this is read
    /// straight into the inequality with no conversion — and with no rescaling either,
    /// which is the caveat the type's documentation states.
    NanosPerByte(f64),
}

impl Rival {
    /// What one confirming pass costs per byte.
    ///
    /// `accelerator` is what the engine said it will skip past, or `None` where the
    /// automaton would not name a start state to ask about. Read by [`Rival::Engine`]
    /// alone — the other two carry their own price and are indifferent to the automaton,
    /// which is the entire point of them.
    ///
    /// # A nonsense price is refused at the comparison, not here
    ///
    /// Neither float variant is validated. That is a placement decision rather than an
    /// omission: a price that is not a price has to be refused wherever it comes from,
    /// and a caller can also reach the gate with a hand-built [`Calibration`] carrying
    /// the same defect. So the guard lives once in [`CostFact::pays`], which requires
    /// both sides of the inequality to be real costs before comparing them, and covers
    /// every source of the problem instead of the two that happen to be spelled here.
    ///
    /// It is worth being exact about why this is not merely tidier. A negative rival
    /// does **not** fail closed on its own — it inverts the inequality and arms a leaky
    /// filter — so validating in this function and calling the matter settled would have
    /// left the identical hole open one constructor over.
    ///
    /// [`CostFact::pays`]: super::CostFact::pays
    #[must_use]
    pub fn per_byte(
        self,
        cal: &Calibration,
        accelerator: Option<&[u8]>,
        freq: &[f64; 256],
        at: Residency,
    ) -> f64 {
        match (self, accelerator) {
            (Self::Engine, Some(accel)) => cal.rival_per_byte(accel, freq, at),
            // Nothing to read the engine's intent from, so credit it with its cheapest
            // path and let the sieve stand down, rather than arm against an unknown.
            (Self::Engine, None) => cal.dfa_skip[at as usize],
            (Self::Walks(walks), _) => cal.dfa_walk * walks,
            (Self::NanosPerByte(nanos), _) => nanos,
        }
    }
}

/// What the caller would run if this sieve did not exist — the baseline the gate
/// measures a refutation against.
///
/// # The comparison this exists to refuse
///
/// [`Rival`] answers "what does one confirm cost", and for a long time the gate took
/// that answer as the whole right-hand side: the caller pays `rivals * rival` on every
/// document, and the sieve is worth the fraction of that it retires. That is only the
/// caller's real alternative when confirming is the *only* way to decide, and for a
/// crate whose entire subject is regular expressions it usually is not — the pattern
/// has an engine, the engine is in the dependency graph, and the engine answers the
/// same question exactly.
///
/// The arithmetic is not close. Take a confirm at five hundred walks a byte and a
/// filter armed at the gate's own threshold, so roughly four documents in five survive
/// it. Fronting the confirm directly, the sieve looks like a four-hundred-fold
/// improvement over paying five hundred walks on everything. But nobody pays five
/// hundred walks on everything: they run the engine, which is *exact*, so the share of
/// documents reaching the confirm is the true hit rate — a ten-thousandth, for a secret
/// scanner — and the pipeline costs one walk plus a rounding error. The sieve was two
/// orders of magnitude behind the alternative it was never compared against.
///
/// So the gate takes the **cheaper** of the two ([`crate::price::CostFact::unfiltered`]),
/// and this is where a caller says which alternatives they actually have.
///
/// # Why no hit rate appears anywhere
///
/// Because it cancels. A sieve in front of an exact pre-pass changes what the pre-pass
/// costs and nothing downstream of it: the documents that reach the confirm are the ones
/// that truly match, on both sides of the inequality, whether or not a refutation ran
/// first. Every term after the exact decision is identical in both pipelines and drops
/// out of the difference — which is the whole reason this is one number rather than a
/// model of the caller's architecture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bypass {
    /// One engine per rival, priced off the automaton being fronted — `rivals` times
    /// [`Rival::Engine`]. The default, and the one that costs nothing to be right:
    /// under `Rival::Engine` it is exactly the blind pipeline and changes no verdict,
    /// and under a stated rival it is the engine the caller already has.
    Engines,
    /// One exact pre-pass over the whole slate, at a stated price rather than `rivals`
    /// separate ones — a union automaton, Hyperscan, an Aho-Corasick prefilter over the
    /// slate's literals, an index probe.
    ///
    /// Stated because it is not derivable from here, and because it is usually a much
    /// stronger baseline than it sounds. Measured on the machine these paragraphs were
    /// written on, sixty-four literal-prefixed rules cost **11.96 ns/B as separate
    /// engines and 0.12 as one union** — the fan-out, almost exactly, because the union
    /// keeps a multi-literal accelerator and so still pays one pass's price. A sieve
    /// fronting *that* has essentially nothing to retire, which is the answer for most
    /// slates with literals in them.
    ///
    /// What bounds the union is construction rather than throughput, so a caller reaching
    /// for this should know where it stops: over the same rules the dense table grows
    /// 12.6 KiB → 4.5 MiB → 65 MiB at 1, 64 and 256 rules, and the build goes 0.2 ms →
    /// 0.75 s → 114 s. Past 256 it does not determinize inside a gibibyte at all, and
    /// [`Bypass::Engines`] is the only honest baseline left.
    Slate(Rival),
    /// No exact decision procedure can run at this point in the pipeline, so the confirm
    /// really is what a survivor costs.
    ///
    /// The narrow case, and worth being suspicious of: it is not enough that the confirm
    /// be expensive, or that the engine be inconvenient. It has to be *impossible* to
    /// decide the question exactly and cheaply where the sieve runs. Screening packets
    /// against rules whose matches only exist in a reassembled flow is the honest
    /// exemplar — the refutation is per packet, the exact answer is not available per
    /// packet, and there is nothing cheaper to be compared against.
    ///
    /// Prices exactly as the gate did before a baseline existed, which is the other
    /// reason to name it: a caller who believes the old arithmetic can ask for it.
    Absent,
}

impl Bypass {
    /// What the whole alternative pipeline costs per byte, already summed over the
    /// slate — so [`crate::price::CostFact::bypass`] is an absolute price and not
    /// something the gate has to know how to scale.
    ///
    /// `accelerator` is what the automaton said it will skip past, exactly as
    /// [`Rival::per_byte`] reads it.
    #[must_use]
    pub fn per_byte(
        self,
        cal: &Calibration,
        accelerator: Option<&[u8]>,
        freq: &[f64; 256],
        at: Residency,
        rivals: usize,
    ) -> f64 {
        match self {
            Self::Engines => {
                Rival::Engine.per_byte(cal, accelerator, freq, at) * super::gate::fanout(rivals)
            },
            Self::Slate(one) => one.per_byte(cal, accelerator, freq, at),
            Self::Absent => f64::INFINITY,
        }
    }
}

/// What share of the corpus a byte set covers under `freq`, clamped to a probability.
///
/// Factored out because both blends above need exactly it, and a set that summed past
/// one — a caller's own marginals need not be normalized — would make an escape rate
/// into a negative residency.
fn share(set: &[u8], freq: &[f64; 256]) -> f64 {
    set.iter()
        .map(|&b| freq[usize::from(b)])
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::{CostFact, MACOS_AARCH64_NEON, NOMINAL_LEN};
    use crate::prior;

    const UNMINTED: Calibration = UNMEASURED;
    const REGIME: [Residency; REGIMES] = Residency::ALL;

    #[test]
    fn a_target_nobody_measured_is_infinite_never_free() {
        for n in 1..=MAX_CONJUNCTS {
            assert!(
                UNMINTED.sieve_per_byte(n).is_infinite(),
                "a zero coefficient would pass every worth test"
            );
        }
        for at in REGIME {
            assert!(
                !UNMEASURED.is_measured(at),
                "the unmeasured row must admit it is unmeasured, in every regime"
            );
        }
        assert!(MACOS_AARCH64_NEON.is_measured(Residency::Memory));
    }

    /// The regime column is a key, not a hint: a row measured in one regime and not the
    /// other must read as uncalibrated for the one it cannot price, and [`active`] must
    /// hand back [`UNMEASURED`] rather than the column it does have.
    ///
    /// This is the same refusal the crate already makes about a whole machine — "no row
    /// for this silicon, so no promise" — pushed one column in, and it is what keeps a
    /// half-minted row from quietly pricing a cache-resident scan off memory-resident
    /// numbers that are 2x too generous.
    #[test]
    fn a_regime_nobody_measured_declines_instead_of_borrowing_the_other() {
        let half = Calibration {
            dfa_skip: [0.0, 0.015817],
            ..MACOS_AARCH64_NEON
        };
        assert!(half.is_measured(Residency::Memory));
        assert!(
            !half.is_measured(Residency::Cache),
            "an unminted regime must not inherit the minted one"
        );
        // And the shipped rows are consistent with themselves: any regime a row claims
        // to have measured must have every coefficient that regime needs.
        for cal in MINTED {
            for at in REGIME {
                if cal.is_measured(at) {
                    assert!(
                        cal.dfa_excursion[at as usize] > 0.0,
                        "{} claims {at:?} with no excursion coefficient",
                        cal.arch
                    );
                    assert!(
                        cal.skip_excursion.iter().all(|per| per[at as usize] > 0.0),
                        "{} claims {at:?} with an unpriced instrument",
                        cal.arch
                    );
                }
            }
        }
    }

    /// The claim that decouples this crate from one laptop: the gate reads three
    /// dimensionless ratios, so scaling every coefficient — a slower clock, a hotter
    /// die, ten coworker agents on the machine — moves no decision at all. Anything
    /// that broke this would make the arming gate a function of ambient load.
    ///
    /// Swept over every regime the row claims, because the invariance has to hold
    /// *within* a regime and the residency axis is precisely the part of the variation
    /// that scaling does **not** cover. A uniform factor is what a clock or a thermal
    /// state does to a machine; moving a haystack from DRAM into cache rescales two
    /// coefficients and leaves two alone, which is why it needed a dimension instead of
    /// being absorbed here.
    #[test]
    fn scaling_the_whole_calibration_changes_no_decision() {
        let freq = prior::Prior::Source.byte_freq();
        let mut swept = 0;
        for at in REGIME {
            if !MACOS_AARCH64_NEON.is_measured(at) {
                continue;
            }
            swept += 1;
            for k in [0.25f64, 1.0, 3.7, 91.0] {
                let scaled = Calibration {
                    dfa_skip: MACOS_AARCH64_NEON.dfa_skip.map(|c| c * k),
                    dfa_walk: MACOS_AARCH64_NEON.dfa_walk * k,
                    // Dimensionless: an excursion is a count of walk-priced bytes, not
                    // a duration, so it must NOT scale.
                    dfa_excursion: MACOS_AARCH64_NEON.dfa_excursion,
                    sieve: MACOS_AARCH64_NEON.sieve.map(|c| c * k),
                    ..MACOS_AARCH64_NEON
                };
                for accel in [&b""[..], b"W", b"e", b"abg"] {
                    for fallthrough in [0.0, 1e-6, 1e-3, 0.5] {
                        let of = |cal: &Calibration| CostFact {
                            fallthrough,
                            len: NOMINAL_LEN,
                            sieve: cal.sieve_per_byte(MAX_CONJUNCTS),
                            rival: cal.rival_per_byte(accel, &freq, at),
                            rivals: 1,
                            // Read through the seam rather than pinned, so the baseline
                            // rescales with the row exactly as the rival does — a bypass
                            // that did not would break the invariance from the one side
                            // the sweep would otherwise never look at.
                            bypass: Bypass::Engines.per_byte(cal, Some(accel), &freq, at, 1),
                        };
                        let (base, now) = (of(&MACOS_AARCH64_NEON), of(&scaled));
                        assert_eq!(
                            base.pays(),
                            now.pays(),
                            "k={k} at={at:?} accel={accel:?} f={fallthrough} flipped the gate"
                        );
                        assert!(
                            (base.speedup() - now.speedup()).abs() < 1e-9,
                            "k={k} at={at:?} moved the predicted speedup: {} vs {}",
                            base.speedup(),
                            now.speedup()
                        );
                    }
                }
            }
        }
        assert!(swept > 0, "the reference row measured no regime at all");
    }

    /// On the machine running the suite, resolution must either find a row minted for
    /// exactly this (os, architecture, kernel) triple or fall through to the unmeasured
    /// one. Inheriting a foreign row is the failure this pins shut — and the reason the
    /// operating system is in the triple is that this assertion used to be written
    /// without it, so a macOS x86_64 host inheriting a Linux x86_64 row read as a match.
    #[test]
    fn resolution_matches_this_machine_or_admits_it_cannot() {
        for at in REGIME {
            let cal = active(at);
            if cal.is_measured(at) {
                assert_eq!(cal.os, OS);
                assert_eq!(cal.arch, ARCH);
                assert_eq!(cal.kernel, crate::shuffle::kernel());
            } else {
                assert!(
                    !MINTED.iter().any(|c| c.os == OS
                        && c.arch == ARCH
                        && c.kernel == crate::shuffle::kernel()
                        && c.is_measured(at)),
                    "a row exists for this machine in {at:?} but resolution missed it"
                );
            }
        }
    }

    /// [`ARCH`] replaced `std::env::consts::ARCH`, and the replacement is only
    /// correct if it is the *same string* — a mismatch would resolve every machine to
    /// [`UNMEASURED`] and silently disarm the crate rather than fail loudly. Checked
    /// against `std` wherever there is a `std` to check against.
    #[cfg(feature = "std")]
    #[test]
    fn the_cfg_derived_arch_is_the_one_the_standard_library_reports() {
        assert_eq!(ARCH, std::env::consts::ARCH);
    }

    /// The same guard for [`OS`], and it needs one more badly than [`ARCH`] does: this
    /// column is deliberately *not* exhaustive — an operating system nobody has minted on
    /// is meant to read `"unknown"` — so the failure mode is not a typo resolving to
    /// nothing, it is a typo in one of the five enumerated arms silently reading as the
    /// catch-all and disarming a machine that does have a row.
    ///
    /// Which is why the comparison is conditioned twice rather than asserted flat, and
    /// both conditions are cases the crate really runs in. `"unknown"` is not a failure —
    /// on FreeBSD it is the correct and intended answer, and demanding equality with
    /// `std` there would fail a target that is behaving exactly as designed. And `std`
    /// itself is not an oracle everywhere: under `wasi` `std::env::consts::OS` is the
    /// *empty string*, while the `cfg`-derived name is `"wasi"` and is what a row would be
    /// keyed on, so equality there would assert that a machine must misname itself.
    #[cfg(feature = "std")]
    #[test]
    fn an_enumerated_os_is_spelled_the_way_the_standard_library_spells_it() {
        assert!(
            !OS.is_empty(),
            "a row keyed on the empty string would match nothing and say nothing"
        );
        let reported = std::env::consts::OS;
        if OS != "unknown" && !reported.is_empty() {
            assert_eq!(OS, reported, "an enumerated arm is misspelled");
        }
    }

    /// [`Rival::Engine`] must be *exactly* the arithmetic it was factored out of.
    ///
    /// This seam arrived under an existing default, so the whole of its risk is here
    /// rather than in the two new variants: a default that shifted by any amount would
    /// silently re-price every verdict the crate has ever taken, and every row
    /// `examples/survey.rs` audits, without a single caller having asked for anything.
    #[test]
    fn the_engine_variant_prices_exactly_what_reading_the_automaton_did() {
        let freq = prior::Prior::Source.byte_freq();
        for at in REGIME {
            for accel in [&b""[..], b"W", b"e", b"abg"] {
                assert_eq!(
                    Rival::Engine.per_byte(&MACOS_AARCH64_NEON, Some(accel), &freq, at),
                    MACOS_AARCH64_NEON.rival_per_byte(accel, &freq, at),
                    "{at:?} accel={accel:?}"
                );
            }
            // An automaton that will not name a start state is the one case this variant
            // answers with no accelerator to read. It credits the engine with its
            // cheapest path, which stands the sieve down rather than arming it against
            // an unknown — the direction every other estimate here also errs in.
            assert_eq!(
                Rival::Engine.per_byte(&MACOS_AARCH64_NEON, None, &freq, at),
                MACOS_AARCH64_NEON.dfa_skip[at as usize],
                "{at:?}"
            );
        }
    }

    /// The documented difference between the two stated rivals, made measurable instead
    /// of promised.
    ///
    /// [`Rival::Walks`] is a multiple of a coefficient, so it rescales with the row and
    /// `scaling_the_whole_calibration_changes_no_decision` keeps holding.
    /// [`Rival::NanosPerByte`] is a duration taken on some other clock, so it does not —
    /// and that is asserted here rather than merely admitted in prose, for the same
    /// reason `prior::Prior::Text` is kept as a superseded model: a limitation nobody
    /// measures is a limitation nobody notices growing.
    #[test]
    fn only_the_dimensionless_rival_survives_rescaling_the_calibration() {
        let freq = prior::Prior::Source.byte_freq();
        let at = Residency::Memory;
        let by = |k: f64| Calibration {
            dfa_skip: MACOS_AARCH64_NEON.dfa_skip.map(|c| c * k),
            dfa_walk: MACOS_AARCH64_NEON.dfa_walk * k,
            // Dimensionless already, so it must not scale. See the sweep above.
            dfa_excursion: MACOS_AARCH64_NEON.dfa_excursion,
            sieve: MACOS_AARCH64_NEON.sieve.map(|c| c * k),
            ..MACOS_AARCH64_NEON
        };
        let speedup = |cal: &Calibration, rival: Rival| {
            CostFact {
                fallthrough: 1e-4,
                len: NOMINAL_LEN,
                sieve: cal.sieve_per_byte(1),
                rival: rival.per_byte(cal, None, &freq, at),
                rivals: 1,
                // The rival term in isolation, which is what this test is about: any
                // finite baseline is cheaper than fifty walks and would clamp it, so
                // the scaling question would never reach the term being asked about.
                bypass: f64::INFINITY,
            }
            .speedup()
        };
        let (base, hot) = (by(1.0), by(4.0));

        let ratio = (speedup(&base, Rival::Walks(50.0)) - speedup(&hot, Rival::Walks(50.0))).abs();
        assert!(
            ratio < 1e-9,
            "a rival stated as walks moved the verdict by {ratio:e} under a rescaling \
             that cancels — it is not dimensionless after all"
        );
        let duration = (speedup(&base, Rival::NanosPerByte(5.0))
            - speedup(&hot, Rival::NanosPerByte(5.0)))
        .abs();
        assert!(
            duration > 1e-3,
            "a rival stated in nanoseconds survived a rescaling of everything it is \
             compared against, so the caveat on the variant is describing nothing"
        );
    }

    /// A count of bytes is a fact the caller has; which regime it puts them in is
    /// arithmetic. The boundary is inclusive of cache on purpose — at exactly the
    /// last-level cache size the working set still fits.
    #[test]
    fn a_working_set_past_last_level_cache_reads_as_memory_resident() {
        for (bytes, want) in [
            (0, Residency::Cache),
            (RESIDENT_ABOVE, Residency::Cache),
            (RESIDENT_ABOVE + 1, Residency::Memory),
            (usize::MAX, Residency::Memory),
        ] {
            assert_eq!(Residency::of_working_set(bytes), want, "{bytes} bytes");
        }
    }

    #[test]
    fn extrapolation_errs_high_in_both_directions() {
        // Measured at two conjuncts only — the shape ACTIVE actually ships.
        let cal = Calibration {
            sieve: [0.0, 0.5],
            ..UNMINTED
        };
        // Downward: never cheaper than the count that was measured.
        assert_eq!(cal.sieve_per_byte(1), 0.5);
        assert_eq!(cal.sieve_per_byte(2), 0.5);

        // Upward: each further pass doubles.
        let cal = Calibration {
            sieve: [0.5, 0.0],
            ..UNMINTED
        };
        assert_eq!(cal.sieve_per_byte(1), 0.5);
        assert_eq!(cal.sieve_per_byte(2), 1.0);
    }
}
