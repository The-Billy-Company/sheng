//! How often a quotient accepts, decided before the scan starts.
//!
//! A filter that rejects nothing is not neutral — it is pure addition on every
//! byte, and its worst case is the case where everything survives to verification.
//! Harvested quotients are **bimodal** rather than mediocre: on a real slate some
//! retire well over 99% of positions while others retire a third, with nothing in
//! between that a single policy could straddle. So "is this worth arming?" has to
//! be answered at build time, and it must be answered without a calibration
//! haystack, because a filter that needs a sample of the document it is about to
//! filter has already paid for the scan.
//!
//! # The chain is joint, and that is the whole correction
//!
//! A quotient plus a byte distribution is a Markov chain, and the naive version
//! puts that chain on the quotient's 16 blocks alone. Doing so silently assumes
//! successive bytes are independent, which prices a `k`-byte class requirement as
//! `p^k` and is wrong by orders of magnitude on real text (see [`crate::prior`]).
//!
//! The fix is to carry the byte's class in the state: the chain runs on
//! **(block, class)** pairs, so the next byte's distribution depends on the last
//! byte's class exactly as the persistence matrix says it should. A run of digits
//! stays in the digit rows, where digits are common, instead of being re-drawn from
//! the marginal every position. Under a memoryless prior the joint chain collapses
//! back to the naive one exactly, so this is a strict generalization rather than a
//! different model.
//!
//! The **Cesàro average** of the power iteration is what gets summed — the long-run
//! fraction of positions spent in each state — because a quotient can be periodic,
//! and an ordinary limit would oscillate forever instead of converging.
//!
//! # Why the iteration never touches a byte
//!
//! The chain's alphabet is seven classes; the quotient's is 256 bytes. Stepping the
//! chain byte by byte therefore re-derives, five hundred times, a distribution that
//! only ever had class resolution — and it costs `blocks × classes × 256` multiplies
//! a step, which measured at **~50 ms per `Sieve::new`** and made the build, not the
//! scan, the expensive half of this crate.
//!
//! [`Spread`] is that arithmetic done once. Bytes of one class are interchangeable
//! to the prior, so all that survives aggregation is *where a class can carry a
//! block and what share of the class goes there* — typically one or two edges where
//! the byte loop walked 256. The step then factors in two, because the byte's class
//! is the only thing both halves share:
//!
//! ```text
//! g[block][c]  = Σ_from  p[block][from] · next[from][c]      one 7x7 matmul
//! p'[to][c]   += g[block][c] · share(block, c, to)           one pass over the edges
//! ```
//!
//! Identical mathematics — [`Spread::of`] is a regrouping of the same sum, and the
//! test below re-runs the byte-level definition and demands agreement — at roughly a
//! thirtieth of the arithmetic. An aperiodic chain then stops as soon as it reaches
//! a fixed point, because from there every remaining Cesàro term is the term already
//! in hand and can be added in closed form rather than iterated for.

use alloc::vec::Vec;

use crate::lattice::{LANES, Quotient};
use crate::prior::{CLASSES, Chain, Class, members};

/// Joint chain width: one slot per (block, class) pair.
const JOINT: usize = LANES * CLASSES;

/// Power-iteration steps. The chain has at most 112 states, so this is far past
/// mixing for any aperiodic one and leaves the Cesàro average enough terms to
/// average out a short cycle in a periodic one.
///
/// Every one of them is taken. Stopping at a fixed point is the obvious saving and
/// it is **wrong here**: the answer is the accepting tail, a quantity that routinely
/// sits twenty orders of magnitude under the bulk mass and is still climbing at the
/// step where the bulk stops moving. Any tolerance loose enough to fire is loose
/// enough to be blind to the entire result — the differential test below caught
/// exactly that, reading zero where the byte-level walk reads `1.2e-29`. So the
/// saving is taken out of the *step*, where it is free, rather than the count.
const ITERATIONS: usize = 512;

const fn slot(block: u8, class: usize) -> usize {
    block as usize * CLASSES + class
}

/// Every byte's class index, resolved at compile time.
const CLASS_OF: [usize; 256] = {
    let mut t = [0usize; 256];
    let mut b = 0u8;
    loop {
        t[b as usize] = Class::of(b) as usize;
        if b == u8::MAX {
            break t;
        }
        b += 1;
    }
};

/// Where one byte class can carry one block, and what share of the class goes there.
///
/// The shares in a span sum to 1 by construction: every byte value of a class lands
/// somewhere, and a class's members are equiprobable under any [`Chain`], which is
/// exactly why the 256-byte loop had nothing left to say once its result was grouped.
#[derive(Clone, Copy)]
struct Edge {
    to: u8,
    share: f64,
}

/// A quotient's transition table regrouped to the resolution the prior actually has.
///
/// Built once per quotient and reused across every [`Chain`] the gate sweeps, because
/// where a class of bytes leads is a fact about the *automaton* — the chain only
/// decides how much mass takes each class, never which block a class reaches.
pub struct Spread {
    blocks: usize,
    start: u8,
    threshold: u8,
    /// Successor edges for every (block, class), concatenated.
    edges: Vec<Edge>,
    /// `[block][class]` → the half-open range of [`Spread::edges`] it owns.
    spans: [[(u16, u16); CLASSES]; LANES],
}

impl Spread {
    /// Group `q`'s 256 byte columns into per-class successor sets. One pass over the
    /// table per block — the last time any of this arithmetic looks at a byte.
    #[must_use]
    pub fn of(q: &Quotient) -> Self {
        let blocks = usize::from(q.blocks);
        let mut edges = Vec::with_capacity(blocks * CLASSES * 2);
        let mut spans = [[(0u16, 0u16); CLASSES]; LANES];
        for (block, spans_of) in spans[..blocks].iter_mut().enumerate() {
            let mut tally = [[0u32; LANES]; CLASSES];
            for (row, &class) in q.rows.iter().zip(&CLASS_OF) {
                tally[class][usize::from(row[block])] += 1;
            }
            for ((span, seen), class) in spans_of.iter_mut().zip(&tally).zip(Class::ALL) {
                // A span is at most LANES edges long and there are at most
                // LANES * CLASSES spans, so the whole table indexes in a u16.
                let from = edges.len() as u16;
                for (to, &n) in seen[..blocks].iter().enumerate() {
                    if n > 0 {
                        edges.push(Edge {
                            to: to as u8,
                            share: f64::from(n) / members(class),
                        });
                    }
                }
                let until = edges.len() as u16;
                *span = (from, until);
            }
        }
        Self {
            blocks,
            start: q.start,
            threshold: q.threshold,
            edges,
            spans,
        }
    }

    /// The long-run fraction of byte positions at which the quotient accepts, under
    /// `chain`.
    #[must_use]
    pub fn rate(&self, chain: &Chain) -> f64 {
        // Two buffers alternated by reference rather than by value: these are kilobyte
        // arrays, and exchanging them bodily would memcpy more per step than the step
        // itself spends on arithmetic.
        let (mut this, mut that) = ([0.0f64; JOINT], [0.0f64; JOINT]);
        let (mut held, mut next) = (&mut this, &mut that);

        // The scan starts in the quotient's start block, with the first byte's class
        // drawn from the stationary distribution. A block's classes are contiguous in
        // `slot` order, so seeding is one copy.
        let start = slot(self.start, 0);
        held[start..start + CLASSES].copy_from_slice(&chain.start);

        // Accepting blocks are the top ones by construction, so their joint slots are
        // one contiguous tail — and the Cesàro average of that tail is the average of
        // its per-step sums, so the running total is a scalar rather than a vector
        // summed at the end.
        let live = self.blocks * CLASSES;
        let tail = slot(self.threshold, 0);
        let memoryless = chain.marginal();
        let mut cesaro = 0.0f64;
        for _ in 0..ITERATIONS {
            next[..live].fill(0.0);
            for (block, spans_of) in self.spans[..self.blocks].iter().enumerate() {
                let was = &held[block * CLASSES..][..CLASSES];
                // Everything the chain has to say about this block, in one 7x7 product:
                // how much of the block's mass draws a byte of each class next,
                // whatever class it arrived on. The quotient is not consulted yet, and
                // never again after.
                let mut draw = [0.0f64; CLASSES];
                match memoryless {
                    // Nothing about the arrival class survives into the draw, so the
                    // block's whole mass can be totalled once and scaled once.
                    Some(marginal) => {
                        let mass: f64 = was.iter().sum();
                        for (d, &p) in draw.iter_mut().zip(marginal) {
                            *d = mass * p;
                        }
                    },
                    None => {
                        for (row, &mass) in chain.next.iter().zip(was) {
                            if mass == 0.0 {
                                continue;
                            }
                            for (d, &p) in draw.iter_mut().zip(row) {
                                *d += mass * p;
                            }
                        }
                    },
                }
                for (class, (&(from, until), mass)) in spans_of.iter().zip(draw).enumerate() {
                    if mass == 0.0 {
                        continue;
                    }
                    // The byte drawn *is* the next state's class, so the class index is
                    // fixed across the span and only the block moves.
                    for edge in &self.edges[usize::from(from)..usize::from(until)] {
                        next[slot(edge.to, class)] += mass * edge.share;
                    }
                }
            }
            core::mem::swap(&mut held, &mut next);
            cesaro += held[tail..live].iter().sum::<f64>();
        }
        cesaro / ITERATIONS as f64
    }
}

/// The pessimistic fallthrough rate over every chain in `chains` — what the gate
/// decides on. A conjunction is scored on its best member, because a document only has
/// to be refuted once.
///
/// Sweeping a set and taking the maximum is what makes an unknown corpus safe: adding
/// a chain can only raise the estimate, so it can only make the gate stricter. An
/// empty `chains` therefore has no opinion and returns a fully-permissive `0.0`, which
/// is why [`crate::Policy`] never hands one over.
#[must_use]
pub fn worst_case(quotients: &[Quotient], chains: &[Chain]) -> f64 {
    quotients
        .iter()
        .map(|q| {
            let spread = Spread::of(q);
            chains
                .iter()
                .map(|chain| spread.rate(chain))
                .fold(0.0f64, f64::max)
        })
        .fold(1.0f64, f64::min)
}

/// Held against the byte-level definition of the same chain, which needs real
/// quotients off real patterns — so this module builds automata, and is therefore
/// gated on the feature that can.
#[cfg(all(test, feature = "regex-automata"))]
mod tests {
    use regex_automata::dfa::dense;
    use regex_automata::nfa::thompson;
    use regex_automata::util::syntax;

    use super::*;
    use crate::{lattice, prior, projection};

    fn quotients(pattern: &str) -> Vec<Quotient> {
        let dfa = dense::Builder::new()
            .syntax(syntax::Config::new().utf8(false))
            .thompson(thompson::Config::new().utf8(false))
            .build(pattern)
            .expect("pattern builds");
        let core = projection::Projection::of(&dfa).expect("projects");
        lattice::harvest(&core)
    }

    /// The byte-level definition of the joint chain, transcribed straight from
    /// [`Chain::bytes_after`] with no grouping at all.
    ///
    /// This is the model as [`crate::prior`] states it, and it is the *specification*
    /// [`Spread`] claims to regroup rather than change — so it is written out here
    /// independently instead of being expressed in terms of the thing under test.
    fn by_byte(q: &Quotient, chain: &Chain) -> f64 {
        let weights: Vec<[f64; 256]> = Class::ALL.iter().map(|&c| chain.bytes_after(c)).collect();
        let mut p = [0.0f64; JOINT];
        let start = slot(q.start, 0);
        p[start..start + CLASSES].copy_from_slice(&chain.start);
        let mut cesaro = [0.0f64; JOINT];
        for _ in 0..ITERATIONS {
            let mut next = [0.0f64; JOINT];
            for block in 0..q.blocks {
                for (from, row) in weights.iter().enumerate() {
                    let mass = p[slot(block, from)];
                    if mass == 0.0 {
                        continue;
                    }
                    for (byte, &weight) in row.iter().enumerate() {
                        if weight != 0.0 {
                            let to = q.rows[byte][usize::from(block)];
                            next[slot(to, CLASS_OF[byte])] += mass * weight;
                        }
                    }
                }
            }
            p = next;
            for (c, &x) in cesaro.iter_mut().zip(&p) {
                *c += x;
            }
        }
        let iterations = ITERATIONS as f64;
        cesaro[slot(q.threshold, 0)..slot(q.blocks, 0)]
            .iter()
            .sum::<f64>()
            / iterations
    }

    /// The regrouping is not an approximation. Every quotient a real slate harvests,
    /// under every shipped prior, must land where the byte-level walk lands — and the
    /// rates span twenty orders of magnitude, so the comparison is relative.
    #[test]
    fn grouping_the_bytes_by_class_computes_the_same_chain() {
        const SLATE: &[&str] = &[
            r"(?-u)WalletService",
            r"(?-u)[0-9]{3}-[0-9]{4}",
            r"(?-u)(alpha|beta|gamma)",
            r"(?-u)<[^>]*>",
            r"(?-u)#[0-9a-fA-F]{6}",
            r"(?-u)a[^\n]*b",
        ];
        let mut checked = 0usize;
        for pattern in SLATE {
            for q in quotients(pattern) {
                let spread = Spread::of(&q);
                for chain in &prior::DEFAULT_CHAINS {
                    let (fast, slow) = (spread.rate(chain), by_byte(&q, chain));
                    let scale = slow.abs().max(fast.abs()).max(f64::MIN_POSITIVE);
                    assert!(
                        (fast - slow).abs() / scale < 1e-9,
                        "{pattern:?}: grouped {fast:e} against by-byte {slow:e}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(
            checked >= 12,
            "only {checked} comparisons — the slate harvested nothing"
        );
    }

    /// The property that makes the shipped prior set safe to *grow*: sweeping more
    /// corpora can only raise the fallthrough estimate, so it can only tighten the
    /// gate, never arm something that was declining.
    ///
    /// Worth pinning rather than reading off the `fold`, because it is what the whole
    /// [`prior::DEFAULT_CHAINS`] design rests on. It is why a prior minted over a
    /// corpus the mint could not fully measure — a row absorbed at the support floor,
    /// a corpus with no non-ASCII byte in it — is still safe to ship in the default
    /// set: the worst case over four corpora is at least the worst case over any one
    /// of them, so a thin row can cost breadth and cannot cost soundness.
    #[test]
    fn sweeping_more_priors_never_lowers_the_fallthrough_it_reports() {
        let mut compared = 0usize;
        for pattern in [
            r"(?-u)WalletService",
            r"(?-u)[0-9]{3}-[0-9]{4}",
            r"(?-u)<[^>]*>",
        ] {
            let qs = quotients(pattern);
            for take in 1..=prior::DEFAULT_CHAINS.len() {
                let (narrow, wide) = (
                    worst_case(&qs, &prior::DEFAULT_CHAINS[..take - 1]),
                    worst_case(&qs, &prior::DEFAULT_CHAINS[..take]),
                );
                assert!(
                    wide >= narrow,
                    "{pattern:?}: {take} chains report {wide:e} against {narrow:e} for {}",
                    take - 1
                );
                compared += 1;
            }
        }
        assert!(
            compared >= 12,
            "only {compared} comparisons — nothing harvested"
        );
    }

    /// Every class's mass has to land somewhere. A span that does not sum to 1 is a
    /// byte value the grouping dropped, which would under-count the fallthrough of
    /// every block it can reach.
    #[test]
    fn every_span_is_a_distribution_over_the_blocks_a_class_can_reach() {
        for q in quotients(r"(?-u)[A-Z][a-z]+Service") {
            let spread = Spread::of(&q);
            for block in 0..spread.blocks {
                for (class, &(from, until)) in spread.spans[block].iter().enumerate() {
                    let sum: f64 = spread.edges[usize::from(from)..usize::from(until)]
                        .iter()
                        .map(|e| e.share)
                        .sum();
                    assert!(
                        (sum - 1.0).abs() < 1e-12,
                        "block {block} class {class} spans {sum} of its bytes"
                    );
                }
            }
        }
    }
}
