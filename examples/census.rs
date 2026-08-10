//! Where is the audience lost? A four-way census over a population of patterns
//! people really grep for, separating the declines the gate *decided* from the ones
//! it never got to see.
//!
//! `survey` audits the patterns that arm; `bench` reports what the stages cost. Both
//! start from a slate chosen because it arms, which is exactly the wrong sample for
//! the question this answers: of the patterns a caller shows up with, what fraction
//! never reaches the economics at all? Four outcomes, and the split between the first
//! two and the last two is the whole point:
//!
//! * **core** — [`Decline::TooWide`]: the reachable core exceeds
//!   [`MAX_CORE_STATES`], so the lattice was never harvested. A bounded repeat
//!   spends a DFA state per count, which is what `{16}` costs.
//! * **register** — [`BuildError::NoQuotient`]: a closed partition exists but none
//!   small enough for a register discriminates. This is the 16-block cap.
//! * **economics** — a sieve was built, priced, and lost. The gate working.
//! * **arms** — a sieve was built, priced, and won.
//!
//! A structural decline is a *ceiling*: no calibration, corpus, or residency moves
//! it, and the pattern is refused before any measurement is consulted. An economic
//! decline is a *verdict*, and re-minting or a different corpus can overturn it. Only
//! one of those two is a reason to change the crate, which is why they are counted
//! apart rather than summed into "most patterns decline".
//!
//! Which raises the question the third scenario answers: an economic decline is a verdict
//! against **a particular rival**, and the rival every other report here assumes is
//! `regex-automata` — one of the fastest byte scanners in existence. Running the same
//! population against a confirm that is not a regex separates "this filter is not good
//! enough" from "this filter is competing with something very hard to beat", and those
//! are different findings that the two-regime census silently merged. See
//! [`Rival`](sheng::Rival).
//!
//! This is a report, not a gate — it asserts nothing about the mix, because the mix
//! is what it exists to measure. Run it before and after anything that claims to
//! widen reach.
//!
//! ```bash
//! cargo run --release --example census
//! ```
//!
//! [`Decline::TooWide`]: sheng::Decline::TooWide
//! [`MAX_CORE_STATES`]: sheng::MAX_CORE_STATES
//! [`BuildError::NoQuotient`]: sheng::BuildError::NoQuotient

use sheng::price::Residency;
use sheng::{BuildError, Bypass, Decline, Gate, Policy, Rival, Sieve};

mod common;

/// One question the census asks: which memory regime, what a survivor costs to
/// confirm, and what the caller would have run instead.
struct Scenario {
    label: &'static str,
    at: Residency,
    rival: Rival,
    bypass: Bypass,
}

/// What a document extraction costs per byte, in dense-DFA walks — the units
/// [`Rival::Walks`] is stated in, so this row says the same thing on every machine.
///
/// A walk is 1.3–2.1 ns/B on the minted machines, so this is a confirm around a
/// microsecond per kilobyte: pulling text out of a PDF or an image, roughly. It is
/// deliberately the *modest* end of the range this variant exists for — an embedding or
/// any other model call is another order of magnitude up, and would make the third block
/// below unanimous and therefore uninformative.
const EXTRACTION: f64 = 512.0;

/// The two regimes against the engine, then the same population twice more against a
/// confirm that is not a regex at all — once with the engine still able to screen for it,
/// and once with nothing able to.
///
/// The first two are one crate-level axis and the last two are a workload axis. The
/// structural counts are identical down all four by construction — no cap consults a
/// price — so every difference that appears is the economics, and the interesting
/// difference is between the third and the fourth rather than between the second and the
/// third. An expensive confirm does not raise the bar a sieve is measured against while
/// something cheaper can decide the same question; it only does so where nothing can.
/// See [`Bypass`](sheng::Bypass).
const SCENARIOS: &[Scenario] = &[
    Scenario {
        label: "Cache-resident, confirmed by the engine",
        at: Residency::Cache,
        rival: Rival::Engine,
        bypass: Bypass::Engines,
    },
    Scenario {
        label: "Memory-resident, confirmed by the engine",
        at: Residency::Memory,
        rival: Rival::Engine,
        bypass: Bypass::Engines,
    },
    Scenario {
        label: "Memory-resident, extraction confirm, engine free to screen for it",
        at: Residency::Memory,
        rival: Rival::Walks(EXTRACTION),
        bypass: Bypass::Engines,
    },
    Scenario {
        label: "Memory-resident, extraction confirm, nothing able to screen",
        at: Residency::Memory,
        rival: Rival::Walks(EXTRACTION),
        bypass: Bypass::Absent,
    },
];

/// The population, by family. Chosen to look like what somebody arrives with rather
/// than what this crate is good at — every family here is a real product surface
/// (secret scanning, PII detection, log triage, code search), and the families are
/// named so a shifted total can be attributed to one of them instead of read as a
/// change in the crate's luck.
const POPULATION: &[(&str, &str)] = &[
    // Literal prefix, then a bounded run of a distinctive alphabet. The shape of
    // essentially every credential in circulation.
    ("secret", r"(?-u)AKIA[0-9A-Z]{16}"),
    ("secret", r"(?-u)ghp_[0-9A-Za-z]{36}"),
    ("secret", r"(?-u)sk_live_[0-9A-Za-z]{24}"),
    ("secret", r"(?-u)npm_[0-9A-Za-z]{36}"),
    ("secret", r"(?-u)xox[baprs]-[0-9A-Za-z-]{10,48}"),
    ("secret", r"(?-u)AIza[0-9A-Za-z_-]{35}"),
    ("secret", r"(?-u)-----BEGIN [A-Z ]+PRIVATE KEY-----"),
    // No literal prefix at all: the alphabet and the separators are the whole
    // signal, which is the shape this crate is theoretically best at.
    ("pii", r"(?-u)[0-9]{3}-[0-9]{2}-[0-9]{4}"),
    ("pii", r"(?-u)[0-9]{3}-[0-9]{4}"),
    ("pii", r"(?-u)4[0-9]{12}(?:[0-9]{3})?"),
    (
        "pii",
        r"(?-u)[0-9]{4}[ -]?[0-9]{4}[ -]?[0-9]{4}[ -]?[0-9]{4}",
    ),
    (
        "pii",
        r"(?-u)\+?[0-9]{1,3}[ .-]?\(?[0-9]{3}\)?[ .-]?[0-9]{3}[ .-]?[0-9]{4}",
    ),
    (
        "log",
        r"(?-u)[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}",
    ),
    (
        "log",
        r"(?-u)[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}",
    ),
    (
        "log",
        r"(?-u)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
    ),
    ("log", r"(?-u)\[(ERROR|WARN|FATAL)\]"),
    ("log", r"(?-u)status=[45][0-9]{2}"),
    ("log", r"(?-u)(GET|POST|PUT|DELETE) /[a-z/]*"),
    ("code", r"(?-u)panic!\("),
    ("code", r"(?-u)\bTODO\b"),
    ("code", r"(?-u)unwrap\(\)"),
    ("code", r"(?-u)fn [a-z_]+\("),
    ("code", r"(?-u)#\[derive\([A-Za-z, ]+\)\]"),
    ("code", r"(?-u)WalletService"),
    ("code", r"(?-u)foo[^\n]*bar"),
    ("web", r"(?-u)#[0-9a-fA-F]{6}"),
    ("web", r"(?-u)https?://[A-Za-z0-9./_-]+"),
    (
        "web",
        r"(?-u)[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}",
    ),
    ("web", r"(?-u)eyJ[A-Za-z0-9_-]{10,}"),
    ("web", r"(?-u)[0-9]+\.[0-9]+\.[0-9]+"),
    ("web", r"(?-u)base64,[A-Za-z0-9+/=]{20,}"),
];

/// Which of the four outcomes a pattern reached, ordered so a tally reads from
/// "never priced" to "priced and won".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    /// The core cap, before any harvest.
    Core,
    /// The register cap, after a harvest that found nothing usable.
    Register,
    /// Some other precondition of soundness — quit bytes, or a start state that
    /// already matches. Counted apart because neither is a *cap* anybody could lift.
    Unsound,
    /// Priced and lost.
    Economics,
    /// Priced and won.
    Arms,
}

impl Outcome {
    /// Is this a ceiling rather than a verdict? The one distinction the census exists
    /// to draw: a structural outcome was reached without consulting a single measured
    /// coefficient, so no re-mint can move it.
    const fn structural(self) -> bool {
        matches!(self, Self::Core | Self::Register | Self::Unsound)
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Register => "register",
            Self::Unsound => "unsound",
            Self::Economics => "economics",
            Self::Arms => "arms",
        }
    }
}

/// Every outcome, in tally order — so the summary can sweep them without restating
/// the variant list and drifting from it.
const OUTCOMES: [Outcome; 5] = [
    Outcome::Core,
    Outcome::Register,
    Outcome::Unsound,
    Outcome::Economics,
    Outcome::Arms,
];

/// One pattern's verdict, carrying the family so a shifted total can be attributed, and
/// the ceiling so a decline can be told apart from a *terminal* decline.
struct Row {
    family: &'static str,
    pattern: &'static str,
    outcome: Outcome,
    /// [`CostFact::ceiling`](sheng::price::CostFact::ceiling), where a price was
    /// reached at all. `None` for a structural refusal, which never got one.
    ceiling: Option<f64>,
}

fn main() {
    println!("{}\n", common::host());
    let armed: Vec<usize> = SCENARIOS
        .iter()
        .map(|scenario| {
            println!("=== {} ===", scenario.label);
            let rows = census(scenario);
            for row in &rows {
                println!(
                    "  {:<7} {:<62} {}",
                    row.family,
                    row.pattern,
                    row.outcome.label()
                );
            }
            let arms = summarize(&rows);
            println!();
            arms
        })
        .collect();

    // The two sentences the last two scenarios were added for. A reader who takes
    // nothing else from this example should take that the gate's answer is mostly a
    // statement about how fast `regex-automata` is — and that naming a more expensive
    // confirm does not change that, because the engine is still there to run first.
    let [.., engine, screened, unscreened] = armed[..] else {
        unreachable!("the scenario list is fixed above")
    };
    let total = POPULATION.len();
    println!(
        "{engine} of {total} arm in front of the engine, and {screened} of {total} in front \
         of a confirm costing {EXTRACTION:.0} walks a byte — the same patterns, because a \
         caller holding a regex would run the engine before the extraction, and the gate \
         prices a sieve against what the caller would really have done."
    );
    println!(
        "Take that screen away and {unscreened} of {total} arm. That difference is the whole \
         value of a costly rival, and it is only available to a caller who genuinely cannot \
         decide the question more cheaply where the sieve runs."
    );

    // And the one axis that moves the *structural* count, which no scenario above can:
    // `Policy::relax`. Printed as a pair because the number on its own says nothing —
    // what the crate claims is a delta, and a claim about a delta should be measured by
    // the instrument that reports it rather than remembered from an afternoon.
    let (strict, relaxed) = (ceiling(false), ceiling(true));
    println!(
        "\nCounter relaxation: {strict} of {total} patterns are refused structurally with \
         `Policy::relax` off, {relaxed} with it on — {} rescued from a ceiling no \
         calibration could have moved.",
        strict.saturating_sub(relaxed),
        total = POPULATION.len()
    );
}

/// How many of the population never reach the economics at all, with relaxation on or
/// off.
///
/// The regime is immaterial and any of them would do: a structural refusal is reached
/// before a single coefficient is read, which is the property that makes this countable
/// separately from every verdict above.
fn ceiling(relax: bool) -> usize {
    let policy = Policy {
        gate: Gate::Ungated,
        relax,
        ..Policy::new(Residency::Memory)
    };
    POPULATION
        .iter()
        .filter(|&&(_, pattern)| {
            matches!(
                Sieve::with(pattern, &policy),
                Err(BuildError::Shape(_) | BuildError::NoQuotient | BuildError::Automaton(_))
            )
        })
        .count()
}

/// Classify every pattern in the population under `at`.
///
/// [`Gate::Ungated`] is what separates the two halves of the answer: it builds
/// whenever a quotient exists and prices it anyway, so an economic loss arrives as
/// `Ok` with [`Sieve::cost`] below the margin rather than as
/// [`BuildError::NotWorthIt`]. Under [`Gate::Worth`] every one of these outcomes is
/// the same `Err`, which is precisely the conflation this example exists to undo.
fn census(scenario: &Scenario) -> Vec<Row> {
    let policy = Policy {
        gate: Gate::Ungated,
        rival: scenario.rival,
        bypass: scenario.bypass,
        ..Policy::new(scenario.at)
    };
    POPULATION
        .iter()
        .map(|&(family, pattern)| {
            let built = Sieve::with(pattern, &policy);
            let outcome = match &built {
                Ok(sieve) if sieve.cost().pays() => Outcome::Arms,
                Ok(_) => Outcome::Economics,
                Err(BuildError::Shape(Decline::TooWide)) => Outcome::Core,
                Err(BuildError::NoQuotient) => Outcome::Register,
                Err(BuildError::Shape(_) | BuildError::Automaton(_)) => Outcome::Unsound,
                // `Ungated` consults no price and no length, so the remaining
                // variants are unreachable here. Naming them is cheaper than a
                // wildcard that would silently absorb a fifth outcome later.
                Err(
                    why @ (BuildError::Uncalibrated { .. }
                    | BuildError::Unmodeled { .. }
                    | BuildError::NotWorthIt(_)),
                ) => unreachable!("an ungated build consulted a price: {why}"),
            };
            Row {
                family,
                pattern,
                outcome,
                ceiling: built.ok().map(|sieve| sieve.cost().ceiling()),
            }
        })
        .collect()
}

/// The tally, and the one sentence it is for. Returns how many armed, which is the only
/// figure that moves between scenarios and therefore the only one worth carrying out.
fn summarize(rows: &[Row]) -> usize {
    let count = |want: Outcome| rows.iter().filter(|row| row.outcome == want).count();
    let total = rows.len();
    let share = |n: usize| 100.0 * n as f64 / total as f64;
    let tally: Vec<String> = OUTCOMES
        .iter()
        .map(|&o| format!("{} {} ({:.0}%)", count(o), o.label(), share(count(o))))
        .collect();
    println!("  outcome of {total}: {}", tally.join(" · "));

    let structural = rows.iter().filter(|row| row.outcome.structural()).count();
    println!(
        "  {structural} of {total} ({:.0}%) never reached the economics — a ceiling, not a verdict.",
        share(structural)
    );

    // Of the ones that were priced and lost, how many were losable at all. A decline
    // whose ceiling is already under the margin cannot be overturned by a longer slate
    // or a costlier confirm — the survival term has spent the whole budget — so it is a
    // different finding from a decline with headroom left, and summing them is what
    // makes "most patterns decline" sound like one fact instead of two.
    let lost = rows.iter().filter(|row| row.outcome == Outcome::Economics);
    let terminal = lost
        .filter(|row| row.ceiling.is_some_and(|c| c < 1.0 + sheng::price::MARGIN))
        .count();
    let priced = count(Outcome::Economics);
    println!(
        "  of the {priced} priced and lost, {terminal} are terminal: no slate size and no \
         rival price reaches the margin from their survival."
    );

    // Per family, because "which product surface is this crate refusing" is the
    // actionable reading and the totals hide it.
    let mut families: Vec<&str> = rows.iter().map(|row| row.family).collect();
    families.dedup();
    for family in families {
        let of = || rows.iter().filter(|row| row.family == family);
        println!(
            "    {family:<7} {}/{} arm, {} refused structurally",
            of().filter(|row| row.outcome == Outcome::Arms).count(),
            of().count(),
            of().filter(|row| row.outcome.structural()).count()
        );
    }
    count(Outcome::Arms)
}
