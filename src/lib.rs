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
//! anything else satisfying [`Dfa`] — projects it onto its
//! reachable core (`projection`), and climbs the lattice of
//! **substitution-property partitions** past the point where language is
//! preserved (`lattice`) — a partition closed under the transition function
//! (`p ≡ q` ⟹ `δ(p,b) ≡ δ(q,b)` for every byte), per Hartmanis & Stearns,
//! *Algebraic Structure Theory of Sequential Machines* (Prentice-Hall, 1966),
//! ch. 2. A closed partition induces a quotient automaton;
//! marking a block accepting whenever any member state accepts makes that
//! quotient recognize a *superset* of the pattern's language. A superset that
//! rejects therefore proves the original rejects. The quotient's own arithmetic is
//! re-derived and re-checked before it is trusted, so a partition that is not
//! actually closed is discarded rather than shipped.
//!
//! # Why it is fast
//!
//! A quotient is capped at 16 blocks, which is one SIMD register, so the
//! transition step is a single byte shuffle with no gather and the accept test is
//! a running max ([`shuffle`]). That kernel is Langdale's **Sheng** (2018,
//! shipped in Hyperscan; see
//! <https://branchfree.org/2018/05/25/say-hello-to-my-little-friend-sheng-a-small-but-fast-deterministic-finite-automaton/>),
//! pointed at an over-approximating quotient rather than at the real automaton —
//! which is what lets a machine that must fit in a register front a pattern far
//! too large to fit in one.
//!
//! # Prior art
//!
//! The contract — over-approximate, reject early, verify survivors exactly — is
//! not novel. Luchaup, De Carli, Jha & Bach's DFA-trees (INFOCOM 2014,
//! [doi:10.1109/INFOCOM.2014.6847977](https://doi.org/10.1109/INFOCOM.2014.6847977))
//! is the same idea, and their paper calls its shrunk DFAs "a special case of
//! quotient automaton"; Češka et al. ([arXiv:1904.10786](https://arxiv.org/abs/1904.10786))
//! cascade crude over-approximating NFAs chosen by a traffic model; Hyperscan's
//! `HS_FLAG_PREFILTER` has shipped the superset-plus-confirmation contract for
//! years. What is narrow here is the SP-lattice *harvest* as the source of the
//! approximation, the register-resident conjunction selection, and the
//! training-free gate below. Notably, DFA-trees also measured a clear slowdown
//! when nothing is rejected — the hazard that gate exists to refuse.
//!
//! # Why it sometimes refuses
//!
//! Most patterns get no sieve, and that is the intended behavior. A sieve arms
//! only when the lattice yields a partition small enough to hold in a register,
//! coarse enough to be a real abstraction, and **cheaper than the engine it would
//! front** ([`price`]). That last test is a comparison of two measured per-byte
//! costs, not a threshold on selectivity — because the decisive question is often
//! not how much the filter rejects but how little the rival costs. When
//! `regex-automata` can `memchr` its way through a document, nothing that inspects
//! every byte can front it profitably, however selective. Such a pattern gets
//! [`BuildError::NotWorthIt`] carrying the arithmetic instead of a slow sieve.
//!
//! Selectivity itself is predicted from the quotient's own Markov chain with no
//! calibration haystack (`selectivity`), under a first-order model of byte-class
//! persistence ([`prior`]) — because an independent-draw model prices a `k`-byte run
//! as `p^k` and is wrong by orders of magnitude on real text.
//!
//! # What is measured, and where measurements stop applying
//!
//! Everything above is arithmetic and instructions; it holds on any machine. The
//! *decision* rests on two empirical facts that are nobody's constants — how fast a
//! machine runs three loops, and what the bytes being searched look like — and
//! [`Policy`] is the single place both live.
//!
//! Absolute speed is provably irrelevant: scaling every coefficient of a
//! [`price::Calibration`] by any positive factor leaves every decision unchanged, so
//! clock, load and thermal state cancel. What does not cancel is three dimensionless
//! ratios, and those turn out to differ about twofold between arm64 and x86_64 — in
//! opposite directions — so [`price::MINTED`] keeps one row per (architecture, kernel)
//! pair that has actually been measured, and a machine absent from it gets
//! [`BuildError::Uncalibrated`] rather than another machine's optimism. The shipped
//! [`prior`] set spans four measured corpora — a polyglot code tree, English prose,
//! machine-generated JSON, sixteen systems' logs — swept together, because the gate
//! takes the worst case over them and a caller who has not said what they are
//! searching should be priced under all four. A caller who *has* narrows to the one
//! that fits; a corpus none of them describe (DNA, a wire protocol) mints its own.
//!
//! # What this crate needs to exist
//!
//! The sieve is arithmetic and sixteen bytes of table, so it asks for very little and
//! says so in its feature set rather than in a paragraph.
//!
//! Scanning needs **no operating system**. [`Sieve::refutes`] reads a [`Quotient`]'s
//! rows and, where one was elected, a [`Skip`] — whose narrowest escape sets go
//! through `memchr`, this crate's one unconditional dependency and itself `no_std`.
//! Pricing needs none either: every float operation in the crate is `+ - * /` and a
//! comparison, so there is no `powf`, no `libm`, and no math library behind either.
//! Even the runtime SSSE3 probe reads `CPUID` directly instead of asking
//! `std::arch::is_x86_feature_detected!`, because for that particular feature bit the
//! two are the same question. `--no-default-features` is therefore a `no_std` build;
//! an allocator is still required, because the tables are [`Vec`]-shaped.
//!
//! Building is where the dependency lives, and only there. A pattern has to be
//! *parsed*, and this crate deliberately does not own a parser — the soundness
//! argument above is about the automaton that will run the confirming
//! search, so reading that engine's automaton is the whole point. The default
//! `regex-automata` feature supplies both the parser and the [`Dfa`] impl.
//! Turn it off and the pattern constructors go away, leaving [`Sieve::of_dfa`]
//! over any automaton a caller can walk. That split falls where the code already
//! divided: nothing in `regex-automata` was ever on the scan path.

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
mod selectivity;
pub mod shuffle;
mod skip;

pub use dfa::Dfa;
pub use error::BuildError;
pub use lattice::{MAX_CONJUNCTS, Quotient, harvest};
pub use price::Residency;
pub use projection::{Decline, Projection};
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
    /// Whether to enforce the worth test at all.
    pub gate: Gate,
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
            gate: Gate::Worth,
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
    pub fn with(pattern: &str, policy: &Policy<'_>) -> Result<Self, BuildError> {
        use alloc::string::ToString;

        use regex_automata::dfa::dense;
        use regex_automata::nfa::thompson;
        use regex_automata::util::syntax;

        let dfa = dense::Builder::new()
            .syntax(syntax::Config::new().utf8(false))
            .thompson(thompson::Config::new().utf8(false))
            .build(pattern)
            .map_err(|e| BuildError::Automaton(e.to_string()))?;
        Self::of_dfa_with(&dfa, policy)
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
        // Refuse before doing any work rather than after: an unmeasured machine cannot
        // be talked into a speedup by a well-shaped automaton.
        if policy.gate == Gate::Worth && !policy.calibration.is_measured(policy.residency) {
            return Err(BuildError::Uncalibrated {
                arch: price::ARCH,
                kernel: shuffle::kernel(),
            });
        }
        let core = projection::Projection::of(dfa).map_err(BuildError::Shape)?;
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
        let cost = CostFact {
            fallthrough,
            len: policy.len,
            sieve,
            rival: rival_cost(dfa, policy),
        };
        if policy.gate == Gate::Worth && !cost.pays() {
            return Err(BuildError::NotWorthIt(cost));
        }
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

/// What the engine costs per byte, asked of the engine rather than assumed.
///
/// [`Dfa::accelerator`] on the start state is the engine stating which bytes it
/// will `memchr` past; an empty answer means it is committed to a per-byte walk.
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
// The README's Rust blocks, held to the compiler. `cfg(doctest)` is set only while
// rustdoc is collecting doctests, so this item is never built into the library and
// never reaches the rendered documentation — which is the point: the module docs above
// are the crate's own argument and the README is a second telling of it for a different
// reader, so `#![doc = include_str!(..)]` at the crate root would duplicate every claim
// to buy this. What is bought is the one thing the two tellings must share: a README
// snippet that stops compiling fails a test rather than sitting there as a paragraph
// nobody re-ran. Every `Sieve` constructor in there took one fewer argument than it does
// now, and nothing said so.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct Readme;

fn rival_cost<D: Dfa>(dfa: &D, policy: &Policy<'_>) -> f64 {
    let Some(start) = dfa.start() else {
        // Cannot tell what the engine will do, so assume the best case for it and let
        // the sieve stand down.
        return policy.calibration.dfa_skip[policy.residency as usize];
    };
    policy
        .calibration
        .rival_per_byte(dfa.accelerator(start), policy.freq, policy.residency)
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
