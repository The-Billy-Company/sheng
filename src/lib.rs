//! **sheng** — register-resident refutation sieves for regex.
//!
//! A sieve answers one question, in one direction: *can this document be proven
//! to hold no match?* When the answer is yes it is conclusive and the document
//! never needs to be scanned. When the answer is no it means nothing at all, and
//! a real engine has to run. Nothing here ever reports a match, or a position, or
//! a capture — the asymmetry is the design, not a limitation of it.
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use sheng::Residency;
//!
//! // The regime has no default: only the caller knows where their bytes come from.
//! let sieve = sheng::Sieve::new(r"(?-u)WalletService", Residency::Memory)?;
//! for doc in std::iter::empty::<&[u8]>() {
//!     if sieve.refutes(doc) {
//!         continue; // proven match-free; no engine runs
//!     }
//!     // ... hand `doc` to a real matcher ...
//! }
//! # Ok(()) }
//! ```
//!
//! # Why it is sound
//!
//! A sieve reads a finished DFA — `regex-automata`'s own `dense::DFA` by default, or
//! anything else satisfying [`Dfa`] — projects it onto its reachable core
//! (`projection`), and climbs the lattice of **substitution-property partitions** past
//! the point where language is preserved (`lattice`). The quotient a closed partition
//! induces recognizes a *superset* of the pattern's language, so a quotient that
//! rejects proves the original rejects. `lattice` carries that argument and its
//! citations; a partition that is not actually closed is discarded rather than shipped.
//!
//! # Why it is fast
//!
//! A quotient is capped at 16 blocks, which is one SIMD register, so the
//! transition step is a single byte shuffle with no gather and the accept test is a
//! running max ([`shuffle`]). That is Langdale's **Sheng** kernel pointed at an
//! over-approximating quotient rather than at the real automaton — which is what lets a
//! machine that must fit in a register front a pattern far too large to fit in one.
//!
//! # Why it sometimes refuses
//!
//! Most patterns get no sieve, and that is the intended behavior. A sieve arms only
//! when the lattice yields a partition small enough to hold in a register, coarse
//! enough to be a real abstraction, and **cheaper than the engine it would front**
//! ([`price`]) — two measured per-byte costs compared, not a threshold on selectivity,
//! because the decisive question is often not how much the filter rejects but how
//! little the rival costs. A pattern `regex-automata` can `memchr` its way through gets
//! [`BuildError::NotWorthIt`] carrying the arithmetic instead of a slow sieve.
//!
//! Selectivity itself is predicted from the quotient's own Markov chain with no
//! calibration haystack (`selectivity`), under the first-order model of byte-class
//! persistence [`prior`] documents.
//!
//! # What is measured
//!
//! Everything above is arithmetic and instructions; it holds on any machine. The
//! *decision* rests on two facts that are nobody's constants — how fast a machine runs
//! three loops, and what the bytes being searched look like — and [`Policy`] is the
//! single place both live. Scaling a whole [`price::Calibration`] moves no decision, so
//! a row is a claim about a machine rather than about a clock, and [`price::MINTED`]
//! keeps one per (operating system, architecture, kernel) triple anybody has measured;
//! a machine absent from it gets [`BuildError::Uncalibrated`] rather than another
//! machine's optimism. The shipped [`prior`] chains are swept together at their worst
//! case, since a caller who has not said what they are searching should be priced under
//! all of them; one who *has* narrows to the chain that fits.
//!
//! # What this crate needs to exist
//!
//! Scanning needs **no operating system**. [`Sieve::refutes`] reads a [`Quotient`]'s
//! rows and, where one was elected, a [`Skip`] — whose narrowest escape sets go through
//! `memchr`, this crate's one unconditional dependency and itself `no_std`. Pricing
//! needs none either: every float operation in the crate is `+ - * /` and a comparison,
//! and even the runtime x86 probes read `CPUID` and `XCR0` directly rather than through
//! `std::arch::is_x86_feature_detected!`.
//!
//! Building is where the dependency lives, and only there, because a pattern has to be
//! *parsed* and the soundness argument above is about the automaton that will run the
//! confirming search. The default `regex-automata` feature supplies both the parser and
//! the [`Dfa`] impl; `--no-default-features` is therefore a `no_std` build, and the
//! pattern constructors go with it, leaving [`Sieve::of_dfa`] over any automaton a
//! caller can walk. An allocator is still required, because the tables are
//! [`Vec`]-shaped.

#![cfg_attr(not(feature = "std"), no_std)]
// Nightly-only, and set by nothing but docs.rs (see `[package.metadata.docs.rs]`):
// it is what makes rustdoc label the `regex-automata` items with the feature that
// carries them, rather than rendering them as though they were unconditional.
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

mod arch;
mod dfa;
mod error;
mod lattice;
pub mod price;
pub mod prior;
mod projection;
#[cfg(feature = "regex-automata")]
mod relax;
#[cfg(feature = "regex-automata")]
mod screen;
mod selectivity;
pub mod shuffle;
mod skip;

pub use dfa::Dfa;
pub use error::BuildError;
pub use lattice::{MAX_CONJUNCTS, Quotient, harvest};
pub use price::{Bypass, Residency, Rival};
pub use projection::{Decline, MAX_CORE_STATES, Projection};
#[cfg(feature = "regex-automata")]
#[cfg_attr(docsrs, doc(cfg(feature = "regex-automata")))]
pub use screen::Screen;
pub use selectivity::worst_case;
pub use skip::{Instrument, Skip};

use alloc::vec::Vec;

use price::{Calibration, CostFact};
use prior::Chain;

/// A conjunction of over-approximating quotients, run as one refutation pass.
///
/// Cheap to clone-free share across threads: a sieve is immutable and holds no
/// scan state, so one instance serves every document and every worker. That is a
/// `Send + Sync + 'static` promise, and the assertion at the foot of this file is
/// what keeps a later field from retracting it.
pub struct Sieve {
    lanes: Vec<Lane>,
    cost: CostFact,
}

/// One conjunct, together with how it was decided this conjunct reads a haystack.
///
/// The two kernels are not ranked — they are suited to different automata, and the
/// choice is made per conjunct at build time. [`shuffle::refutes`] composes four
/// slices and reads every byte at the machine's load-port ceiling; a [`skip`] loop
/// reads almost none but walks its excursions one byte at a time. Which is cheaper
/// depends entirely on how long the quotient sits still, so it is priced rather
/// than assumed.
struct Lane {
    quotient: Quotient,
    /// Present only where the calibration said skipping is the cheaper way to read
    /// this particular quotient.
    skip: Option<Skip>,
}

impl Lane {
    /// Choose a kernel for `quotient`, and report what the chosen one costs per byte.
    ///
    /// A skip is admitted on three conditions, and every refusal falls back to a
    /// composition kernel that is always correct:
    ///
    /// 1. the machine has been measured — a skip is a *priced* trade, and an
    ///    unmeasured calibration reads every coefficient as zero, which would elect
    ///    the skip on every pattern by declaring it free;
    /// 2. the resident block does not accept — otherwise the run has already
    ///    answered and there is nothing to skip toward;
    /// 3. [`Skip::of`] could represent the escape set **exactly**;
    /// 4. it prices below the composition kernel.
    ///
    /// Pricing is [`Calibration::skip_per_byte`], the same blend that prices the
    /// engine's accelerator — a skip loop is an accelerated DFA, so there is one
    /// shape of arithmetic here rather than a second cost model to keep honest.
    fn plan(quotient: Quotient, policy: &Policy<'_>, compose: f64) -> (Self, f64) {
        let usable = policy.skip
            && policy.calibration.is_measured(policy.residency)
            && quotient.start < quotient.threshold;
        // Every condition is checked before `Skip::of` reads 256 rows and allocates,
        // so a conjunct the policy already ruled out costs nothing to rule out.
        let priced = usable
            .then(|| Skip::of(&quotient.rows, quotient.start))
            .flatten()
            .map(|s| {
                let cost = policy
                    .calibration
                    .skip_per_byte(&s, policy.freq, policy.residency);
                (s, cost)
            });
        let (skip, cost) = match priced {
            Some((skip, cost)) if cost < compose => (Some(skip), cost),
            _ => (None, compose),
        };
        (Self { quotient, skip }, cost)
    }

    fn refutes(&self, haystack: &[u8]) -> bool {
        match &self.skip {
            Some(skip) => shuffle::refutes_skipping(&self.quotient, skip, haystack),
            None => shuffle::refutes(&self.quotient, haystack),
        }
    }
}

/// The four numbers that explain a sieve: how many passes it makes, how many of
/// those skip rather than compose, what share of positions it is modeled to pass on,
/// and what the gate expected that to be worth. The quotient tables themselves are
/// register images and would print as noise.
impl core::fmt::Debug for Sieve {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sieve")
            .field("conjuncts", &self.lanes.len())
            .field("skipping", &self.skipping())
            .field("fallthrough", &self.cost.fallthrough)
            .field("speedup", &self.cost.speedup())
            .finish()
    }
}

/// Whether to enforce the worth test.
///
/// [`Gate::Ungated`] says the caller wants the sieve whatever its economics —
/// soundness is a property of the quotient construction and must hold on every
/// pattern that harvests one, not only on the ones the cost policy admits. The
/// differential oracles and the calibration mint need it; production callers do
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Build only if the arming inequality says the sieve pays for itself.
    Worth,
    /// Build whenever a quotient exists, regardless of the economics.
    Ungated,
}

/// Every empirical fact the arming decision rests on, in one replaceable place.
///
/// The quotient construction and the kernel are mathematics and instructions — they
/// hold everywhere. The *decision to use them* rests on measurements that are nobody's
/// universal constants: how fast this machine runs three loops, what the bytes being
/// searched look like, and where those bytes are coming from. [`Policy::new`] fills the
/// first two with the best answers this crate shipped with — a calibration matched to
/// the running machine (or [`price::UNMEASURED`], which declines everything) and the
/// four measured corpora of [`prior::DEFAULT_CHAINS`] swept together — and takes the
/// third as its one argument, because it is the one nothing here can determine.
///
/// There is deliberately **no `Default`**. Two of these fields describe facts this
/// crate can probe, and [`Residency`] describes one it cannot: whether the caller's
/// haystacks are arriving from cache or from main memory changes which patterns pay by
/// a large factor, and guessing it silently is how a pattern can arm on a
/// memory-resident mint and lose hard on a cache-resident corpus. A caller states it
/// or gets no sieve.
///
/// A caller whose silicon is not in [`price::MINTED`], or who knows something about
/// their corpus the shipped sweep does not, overrides the field that is wrong rather
/// than living with a shipped answer that quietly describes someone else's laptop:
///
/// ```no_run
/// # use sheng::{Policy, Residency, Sieve, prior::{self, Prior}};
/// let mut policy = Policy::new(Residency::Memory); // a corpus larger than cache
/// policy.len = 4096.0; // documents match the nominal length
/// // Searching logs and nothing else: one measured corpus instead of the worst of
/// // four, which is the only thing that ever *loosens* the gate.
/// let logs = [Prior::Log.chain()];
/// policy.chains = &logs;
/// policy.freq = &prior::LOG_BYTES;
/// let sieve = Sieve::with(r"\bTODO\b", &policy);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Policy<'a> {
    /// Measured per-byte times for this machine. See [`price::active`].
    pub calibration: Calibration,
    /// Where the bytes about to be searched are coming from — the caller's fact, and
    /// the only one in here with no shipped answer. See [`Residency`].
    pub residency: Residency,
    /// Byte-generating models the fallthrough is judged against; the gate takes the
    /// worst, so more chains can only make it stricter. [`prior::Prior::chain`] names
    /// the measured ones individually.
    pub chains: &'a [Chain],
    /// Byte marginals the rival engine's escape set is priced under — how often the
    /// engine's `memchr` will actually trip on the corpus being searched. One table
    /// rather than a swept set, because this term prices the *rival*: the pessimistic
    /// reading is the one where the engine skips most, so a worst case here would be a
    /// best case for the sieve. [`prior::Prior::byte_freq`] has one per corpus.
    pub freq: &'a [f64; 256],
    /// Nominal haystack length the one-time survival cost is amortized over.
    pub len: f64,
    /// What one confirming pass costs — the gate's whole right-hand side, and the term
    /// that decides more verdicts than selectivity does.
    ///
    /// [`Rival::Engine`] by default, which reads the price off the automaton that will
    /// run the confirming search. A caller whose survivors cost something other than a
    /// regex scan — fetched over a network, put through OCR, embedded, indexed — says so
    /// here. [`Rival`] carries which of its two ways to say it to prefer, and which
    /// confirms are expensive enough to be worth saying at all.
    pub rival: Rival,
    /// What the caller would run **instead** of this sieve, and therefore the baseline
    /// the gate has to beat.
    ///
    /// [`Bypass::Engines`] by default, which is the engine the caller already holds,
    /// once per rival. Under [`Rival::Engine`] that is the same number on both sides and
    /// changes nothing; under a stated rival it is what stops the gate comparing a sieve
    /// against a pipeline nobody would run. [`Bypass`] carries the arithmetic, and it is
    /// worth reading before reaching for [`Rival::Walks`] — the two terms are the same
    /// conversation from opposite ends, and stating only the first is how a sieve arms at
    /// two orders of magnitude behind the engine it never got compared to.
    pub bypass: Bypass,
    /// How many searches one refutation lets the caller skip.
    ///
    /// One by default, which is the shape the rest of the arithmetic was written for:
    /// one pattern, one engine, one document. A caller running a rule slate over each
    /// document pays for the pre-pass **once** and for verification once per pattern, and
    /// dividing through, the gate becomes
    ///
    /// ```text
    /// (sieve/rivals  +  survival * rival) * (1 + MARGIN)   <   rival
    /// ```
    ///
    /// so the sieve's own price — the term that declines most near-parity patterns —
    /// falls away as the slate grows, and arming turns on selectivity. A pattern that
    /// misses by a hair at one rival clears comfortably at two.
    ///
    /// # How much this can be worth, which is less than it looks
    ///
    /// All of it, at every slate size, is bounded by [`price::CostFact::ceiling`]: the
    /// term removes `sieve/rivals` and touches nothing else, so the speedup climbs to
    /// `1 / survival` and stops. That bound is what makes the fan-out narrow in practice
    /// rather than transformative — a filter selective enough for the ceiling to be large
    /// is usually already arming at one rival, and a filter marginal enough to need the
    /// fan-out is marginal because its survival is high, which is the thing the fan-out
    /// cannot move. Measured over 4 KiB documents, the band where this term flips a
    /// verdict is roughly one to one-and-a-half times, and the ceiling is worth reading
    /// before assuming a longer slate is the answer.
    ///
    /// It is also bounded by what a single register can cover. One quotient has to
    /// over-approximate every member at once, and a union of even two literal-free rules
    /// routinely harvests nothing at all while four exceeds [`MAX_CORE_STATES`] — so the
    /// automaton this count is declared against is usually a deliberately coarse superset
    /// of one *family* of rules, not the slate's union. `tests/slate.rs` measures where
    /// that wall is.
    ///
    /// # What a caller has to be true for this to be honest
    ///
    /// Two obligations, and neither is checkable from here:
    ///
    /// * **One refutation really must skip them all.** The sieve has to be built from an
    ///   automaton whose language contains every pattern's — a union automaton
    ///   (`dense::DFA::new_many`) is the direct way, via [`Sieve::of_superset_with`].
    ///   A sieve built from one pattern of the slate proves nothing about the others.
    /// * **The priced rival should be the *cheapest* of them.** This term multiplies one
    ///   representative per-byte price by a count, and the engines in a real slate do
    ///   not all cost the same — one with a rare lead byte is an order of magnitude
    ///   cheaper than one committed to a walk. Underestimating the rival can only make
    ///   the sieve decline ([`price`]), so pricing off the cheapest keeps the whole
    ///   inequality erring the way every other term here errs.
    pub rivals: usize,
    /// Whether to enforce the worth test at all.
    pub gate: Gate,
    /// Whether a pattern-string build may also try a **counter-relaxed** superset of
    /// the pattern, and keep it if the gate prices it better.
    ///
    /// On for callers, because the alternative is refusing an entire population of
    /// patterns for a reason that has nothing to do with whether a filter would pay:
    /// a bounded repeat costs a DFA state per count, so `{16}` alone can put the
    /// reachable core past [`MAX_CORE_STATES`] before a single coefficient is read.
    /// Relaxing the bound is sound in the one direction that matters — see
    /// [`Dfa`] for the obligation and `src/relax.rs` for why the transform meets it —
    /// and the strict automaton still prices the rival and still confirms every
    /// survivor.
    ///
    /// A seam rather than a silent improvement, for the same reason [`Policy::skip`]
    /// is one: relaxation can also *cost* selectivity, so the two candidates are
    /// priced against each other and this is how a caller measures that exchange
    /// instead of taking it on faith. Turning it off reproduces the decline a strict
    /// build would have reported.
    ///
    /// Consulted only by the pattern-string constructors, since it is a transform on a
    /// *parse*. A caller on [`Sieve::of_dfa_with`] holds the automaton already and can
    /// hand a superset of it to [`Sieve::of_superset_with`] directly.
    pub relax: bool,
    /// Whether a conjunct may trade the composition kernel for a `skip` loop when
    /// the calibration says the skip is cheaper.
    ///
    /// On for callers. Off for the calibration mint, and only there: the
    /// [`Calibration::sieve_per_byte`] coefficient *means* what the composition
    /// kernel costs, and it is the number a skip is judged against — so a mint that
    /// let its own timings take the skip path would be grading the exchange rate in
    /// the currency it was setting.
    pub skip: bool,
}

impl Policy<'_> {
    /// The shipped answers for everything this crate can measure, and the caller's
    /// answer for the one thing it cannot.
    ///
    /// This replaces the `Default` impl rather than joining it. `Default` would have to
    /// pick a [`Residency`], and both choices are wrong in a way that matters: assuming
    /// [`Residency::Memory`] arms patterns that lose on a cache-resident corpus, and
    /// assuming [`Residency::Cache`] silently withholds real speedups from the callers
    /// this crate is best at. Neither is a default so much as an unstated guess about
    /// somebody else's workload.
    #[must_use]
    pub fn new(residency: Residency) -> Self {
        Self {
            calibration: price::active(residency),
            residency,
            chains: &prior::DEFAULT_CHAINS,
            freq: &prior::SOURCE_BYTES,
            len: price::NOMINAL_LEN,
            rival: Rival::Engine,
            bypass: Bypass::Engines,
            rivals: 1,
            gate: Gate::Worth,
            relax: true,
            skip: true,
        }
    }
}

/// The pattern-string constructors, which are the only part of this crate that needs
/// a parser — and therefore the only part behind the `regex-automata` feature. The
/// automaton constructors below it are always available.
#[cfg(feature = "regex-automata")]
impl Sieve {
    /// Build a sieve for `pattern`, or explain why the pattern gets none.
    ///
    /// `utf8(false)` is set on both the syntax and NFA legs so a byte-oriented
    /// pattern (`(?-u)…`) is buildable — a sieve reasons over bytes, and a
    /// pattern that can match invalid UTF-8 is a legitimate thing to filter for.
    ///
    /// `residency` has no default because nothing here can determine it; see
    /// [`Residency`] and [`Policy::new`].
    pub fn new(pattern: &str, residency: Residency) -> Result<Self, BuildError> {
        Self::with(pattern, &Policy::new(residency))
    }

    /// Build a sieve regardless of whether it pays. For differential oracles and
    /// for calibration, which have to be able to time a kernel the gate would
    /// refuse — including on a machine nothing has been measured on. Not what a
    /// production caller wants.
    ///
    /// Takes no [`Residency`], and cannot need one: a residency selects which column
    /// of a calibration the *gate* reads, and this is the constructor that does not
    /// consult the gate. It prices against [`Residency::Memory`] so the arithmetic it
    /// retains is still readable, and that arithmetic decides nothing.
    pub fn ungated(pattern: &str) -> Result<Self, BuildError> {
        Self::with(
            pattern,
            &Policy {
                gate: Gate::Ungated,
                ..Policy::new(Residency::Memory)
            },
        )
    }

    /// Build a sieve for `pattern` under a caller-supplied [`Policy`] — the seam for
    /// a machine or a corpus this crate never measured.
    ///
    /// Two candidates are assembled and priced, not one, whenever [`Policy::relax`] is
    /// on and the pattern carries a repetition bound: the strict automaton, and a
    /// counter-relaxed superset of it. Both are sound refuters of the same pattern —
    /// only the strict one is ever asked for the rival's price — so the choice between
    /// them is purely economic and is made by the arithmetic the gate already uses.
    /// Neither direction is assumed: relaxation converts a whole population of
    /// patterns from *never priced* to priced, and it can also pass so much more that
    /// the strict filter wins on merit.
    pub fn with(pattern: &str, policy: &Policy<'_>) -> Result<Self, BuildError> {
        Self::of_pattern(pattern, &relax::strict(pattern)?, policy)
    }

    /// [`Sieve::with`] for a caller who has already built the pattern's own automaton
    /// and is going to keep it — [`Screen`], which needs the same automaton afterwards
    /// to confirm with, so building it twice would be a second determinization for a
    /// result it already holds.
    ///
    /// `strict` must be the automaton for `pattern`. It is not re-derived here, and a
    /// mismatched pair would price one search and refute for another.
    pub(crate) fn of_pattern(
        pattern: &str,
        strict: &regex_automata::dfa::dense::DFA<alloc::vec::Vec<u32>>,
        policy: &Policy<'_>,
    ) -> Result<Self, BuildError> {
        let exact = Self::assembled(strict, strict, policy);
        // Priced against `strict`: the relaxed automaton describes what the *filter*
        // may pass, and the engine that will run behind it is still the strict one.
        // Reading the rival off the relaxed automaton would price a search nobody
        // is going to run — and would price it too high, which argues for arming.
        let loose = policy
            .relax
            .then(|| relax::loosened(pattern))
            .flatten()
            .map(|loose| Self::assembled(&loose, strict, policy));

        // Lower total, not higher speedup, and they are the same test: both candidates
        // are weighed against the identical rival. Ties go to the strict automaton,
        // whose fallthrough is a claim about the pattern the caller actually wrote.
        match (exact, loose) {
            (Ok(exact), Some(Ok(loose))) if loose.cost.total() < exact.cost.total() => loose,
            (Ok(exact), _) => exact,
            // The strict automaton's own shape ruled it out — a core wider than the
            // cap is the ordinary way a counted repeat arrives here — and the relaxed
            // one did not. This is the case the transform exists for.
            (Err(_), Some(Ok(loose))) => loose,
            // No second candidate, or none that built either: report the strict
            // automaton's refusal, which is the one about the caller's own pattern.
            (Err(why), _) => return Err(why),
        }
        .admitted(policy)
    }
}

impl Sieve {
    /// Build a sieve for an automaton the caller already has, so the filter and the
    /// confirming search are provably the same automaton — and so the rival's price
    /// is read from the engine that will actually run.
    ///
    /// This is the constructor with no parser behind it, and the only one a `no_std`
    /// build has. See [`Dfa`] for what an automaton has to be able to answer, and for
    /// the obligation a caller takes on by answering it.
    pub fn of_dfa<D: Dfa>(dfa: &D, residency: Residency) -> Result<Self, BuildError> {
        Self::of_dfa_with(dfa, &Policy::new(residency))
    }

    /// [`Sieve::of_dfa`] with a [`Policy`] the caller assembled rather than one
    /// [`Policy::new`] filled in.
    pub fn of_dfa_with<D: Dfa>(dfa: &D, policy: &Policy<'_>) -> Result<Self, BuildError> {
        Self::of_superset_with(dfa, dfa, policy)
    }

    /// Build the sieve from `filter` while pricing — and intending to front — `rival`.
    ///
    /// The obligation is the one [`Dfa`] already states, and this is the constructor
    /// that makes it a parameter instead of an aside: **`filter`'s language must
    /// contain `rival`'s**. Hand over a narrower automaton and a refutation stops
    /// being a proof; nothing here can check it.
    ///
    /// Why it is worth a second constructor: a superset is often the only automaton
    /// small enough to sieve at all. A bounded repeat spends a DFA state per count, so
    /// `AKIA[0-9A-Z]{16}` puts the reachable core past [`MAX_CORE_STATES`] while
    /// `AKIA[0-9A-Z]+` — whose language strictly contains it — is a handful of states
    /// and a sharper quotient. `Sieve::with` performs exactly that exchange for
    /// pattern strings; this is the same seam for a caller who builds automata
    /// themselves, including on `no_std` where there is no parser to relax.
    ///
    /// The two automata are asked different questions, and that is the whole design:
    /// `filter` supplies the states a quotient is harvested from and the fallthrough
    /// the gate believes, `rival` supplies [`Dfa::accelerator`] and therefore the price
    /// the sieve has to beat. Reading the rival's price off a relaxed automaton would
    /// price a search nobody is going to run.
    pub fn of_superset_with<F: Dfa, R: Dfa>(
        filter: &F,
        rival: &R,
        policy: &Policy<'_>,
    ) -> Result<Self, BuildError> {
        Self::assembled(filter, rival, policy)?.admitted(policy)
    }

    /// The worth test, applied once to a finished candidate.
    ///
    /// Separate from [`Sieve::assembled`] because [`Sieve::with`] prices two candidates
    /// against each other before either is admitted, and a gate applied per candidate
    /// would discard the better one for failing a test the pair had not yet been
    /// compared on.
    fn admitted(self, policy: &Policy<'_>) -> Result<Self, BuildError> {
        if policy.gate == Gate::Worth && !self.cost.pays() {
            return Err(BuildError::NotWorthIt(self.cost));
        }
        Ok(self)
    }

    /// Everything except the worth test: preconditions, projection, harvest, lanes,
    /// and the arithmetic that would decide.
    fn assembled<F: Dfa, R: Dfa>(
        filter: &F,
        rival: &R,
        policy: &Policy<'_>,
    ) -> Result<Self, BuildError> {
        // Refuse before doing any work rather than after: an unmeasured machine cannot
        // be talked into a speedup by a well-shaped automaton.
        if policy.gate == Gate::Worth && !policy.calibration.is_measured(policy.residency) {
            return Err(BuildError::Uncalibrated {
                os: price::OS,
                arch: price::ARCH,
                kernel: shuffle::kernel(),
            });
        }
        // NaN is named rather than left to the comparison, which every ordering answers
        // `false` to: an unorderable length belongs with the ones nothing was measured
        // over, not sailing through the arithmetic downstream of here.
        let unmodeled = policy.len < price::VALIDITY_FLOOR || policy.len.is_nan();
        if policy.gate == Gate::Worth && unmodeled {
            return Err(BuildError::Unmodeled {
                len: policy.len,
                floor: price::VALIDITY_FLOOR,
            });
        }
        let core = projection::Projection::of(filter).map_err(BuildError::Shape)?;
        let quotients = lattice::harvest(&core);
        if quotients.is_empty() {
            return Err(BuildError::NoQuotient);
        }
        let fallthrough = selectivity::worst_case(&quotients, policy.chains);
        let compose = policy.calibration.sieve_per_byte(quotients.len());
        // The worst lane, not the mean: `refutes` short-circuits on the first
        // conjunct that answers, so the measured coefficient already describes one
        // pass — and pricing a pair at the cheaper of the two would credit a
        // short-circuit the caller only sometimes gets.
        let mut sieve = 0.0f64;
        let lanes: Vec<Lane> = quotients
            .into_iter()
            .map(|quotient| {
                let (lane, cost) = Lane::plan(quotient, policy, compose);
                sieve = sieve.max(cost);
                lane
            })
            .collect();
        let (rival, bypass) = prices(rival, policy);
        let cost = CostFact {
            fallthrough,
            len: policy.len,
            sieve,
            rival,
            rivals: policy.rivals,
            bypass,
        };
        Ok(Self { lanes, cost })
    }

    /// Does this sieve **prove** `haystack` holds no match?
    ///
    /// `true` is conclusive: skip the document. `false` is not evidence of a
    /// match — it only means this filter could not rule one out.
    #[must_use]
    pub fn refutes(&self, haystack: &[u8]) -> bool {
        self.lanes.iter().any(|lane| lane.refutes(haystack))
    }

    /// The scalar reference path, semantically identical to [`Sieve::refutes`].
    ///
    /// Public on purpose: holding the vector kernel to a reference is the only way
    /// to know the two agree, and a differential test lives outside this crate's
    /// privacy boundary.
    #[must_use]
    pub fn refutes_scalar(&self, haystack: &[u8]) -> bool {
        self.lanes
            .iter()
            .any(|lane| shuffle::scalar(&lane.quotient, haystack))
    }

    /// The modeled share of positions this sieve passes on, under the pessimistic
    /// prior. Lower is better.
    #[must_use]
    pub fn fallthrough(&self) -> f64 {
        self.cost.fallthrough
    }

    /// The arithmetic that admitted this sieve — retained so a caller, a bench, and
    /// the gate can never drift apart on why it armed.
    #[must_use]
    pub fn cost(&self) -> CostFact {
        self.cost
    }

    /// How many quotients are conjoined. Diagnostic — a caller never needs it to
    /// use a sieve correctly.
    #[must_use]
    pub fn conjuncts(&self) -> usize {
        self.lanes.len()
    }

    /// How many conjuncts read their haystack with a `skip` loop rather than the
    /// composition kernel. Diagnostic — `survey` and `bench` report it so a change
    /// in the kernel mix is visible rather than inferred from a moved number.
    #[must_use]
    pub fn skipping(&self) -> usize {
        self.lanes.iter().filter(|l| l.skip.is_some()).count()
    }
}

// The README's Rust blocks, held to the compiler. `cfg(doctest)` is set only while
// rustdoc collects doctests, so this never reaches the library or the rendered docs —
// which is the point: including the README at the crate root would duplicate every
// claim to buy the one thing worth buying, a README snippet that fails a test rather
// than drifting quietly, as every `Sieve` constructor in there once did.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct Readme;

/// The gate's two right-hand-side prices per byte — what one confirming pass costs, and
/// what the caller's cheapest exact alternative costs — both asked of the rival rather
/// than assumed.
///
/// Answered together because they read the same fact off the same automaton, and asking
/// twice would walk the start state twice for one accelerator. The automaton is consulted
/// only where [`Policy::rival`] or [`Policy::bypass`] says the price *is* the automaton's;
/// [`Rival`] and [`Bypass`] own every other case, including what to charge when a DFA will
/// not name a start state to read an accelerator from.
///
/// Priced under the policy's byte marginals alone, where [`selectivity::worst_case`]
/// sweeps every chain — and the asymmetry is deliberate rather than an oversight.
/// The two quantities answer different questions. How often the engine trips over an
/// escape byte is a fact about the corpus that will actually be searched, and that
/// corpus is source text; sweeping it would let the uniform-random prior, which
/// models a document nobody greps, declare the engine fast and stand a real winner
/// down. Whereas a quotient's fallthrough is a claim about the *pattern*, where the
/// prior sweep is protection against the pattern behaving unlike the model.
///
/// Taking the worst case of one and the realistic case of the other is not mixing
/// worlds: it is pessimism where the sieve makes a promise and realism where the
/// rival does.
fn prices<D: Dfa>(dfa: &D, policy: &Policy<'_>) -> (f64, f64) {
    let accelerator = dfa.start().map(|start| dfa.accelerator(start));
    let (cal, freq, at) = (&policy.calibration, policy.freq, policy.residency);
    (
        policy.rival.per_byte(cal, accelerator, freq, at),
        policy
            .bypass
            .per_byte(cal, accelerator, freq, at, policy.rivals),
    )
}

// The thread promise, held by the compiler instead of by inference.
//
// `Send` and `Sync` are auto traits: a type has them because every one of its fields
// does, which is exactly what makes them possible to *lose* in silence. One `Rc`
// handle, one `Cell` memoizing a probe, one raw pointer into a mapped table, and a
// sieve stops crossing thread boundaries — with the breakage landing on the caller who
// believed the documentation rather than on the commit that took it away. Naming the
// surface here moves that failure to the commit that causes it.
//
// A `const` item rather than a `#[test]`, for the same reason the `no_std` job builds
// bare-metal targets: the promise is not conditional on a test build. This is checked
// in every feature combination and on every target this crate compiles for, including
// the ones with no harness to run a test with and no threads to spawn.
//
// `Sieve` and `BuildError` are held to `'static` on top of it. They are the two a
// caller moves *into* a worker rather than merely shares with one — a sieve built once
// and sent to a pool, an error carried back across a join — and neither could do that
// while borrowing from the automaton it was built from. `'static` is the part that says
// `Sieve::of_dfa` hands back a value rather than a view; it is not implied by the
// signature, only by the fields, which is the same thing this whole block is about.
const _: () = {
    const fn shared<T: Send + Sync>() {}
    const fn owned<T: Send + Sync + 'static>() {}

    owned::<Sieve>();
    owned::<BuildError>();

    // The rest of the public surface, in the order the re-exports above declare it.
    shared::<Quotient>();
    shared::<Residency>();
    shared::<Rival>();
    shared::<Bypass>();
    shared::<Decline>();
    shared::<Projection>();
    shared::<Instrument>();
    shared::<Skip>();
    shared::<Gate>();
    shared::<Policy<'static>>();
    shared::<Calibration>();
    shared::<CostFact>();
    shared::<Chain>();
    shared::<prior::Class>();
    shared::<prior::Prior>();
    shared::<shuffle::Kernel>();
};
