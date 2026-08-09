//! The SP-partition lattice harvest: where an over-approximating automaton comes
//! from, and why rejecting on it is sound.
//!
//! A partition of a DFA's states has the **substitution property** (Hartmanis &
//! Stearns, *Algebraic Structure Theory of Sequential Machines*, Prentice-Hall
//! 1966) when it is closed under the transition function: `s ~ t` implies
//! `δ(s,a) ~ δ(t,a)` for every input. A closed partition induces a well-defined
//! **quotient** automaton on its blocks, and marking a block accepting whenever
//! *any* member state accepts makes that quotient recognize a **superset** of the
//! original language. Superset means a quotient-reject is an original-reject,
//! conclusively. That is the entire soundness argument, and it is 1966
//! mathematics.
//!
//! The SP partitions form a lattice under refinement. At its fine end sits the
//! coarsest partition that still separates accepting from non-accepting
//! behavior — the Myhill-Nerode congruence, whose quotient is the minimal DFA
//! for the *same* language. That is what Moore refinement computes, and it is
//! what `regex-automata`'s `minimize.rs` would compute if it were enabled.
//! Climb **past** that point to a strictly coarser closed partition and the
//! quotient accepts a superset: a machine that can refute but never confirm.
//! That is what this module harvests.
//!
//! The two directions need different machinery, which is why this is not Moore
//! minimization with a flag. Refinement *descends*, splitting from the accept
//! partition downward, and wants a signature hash per pass. SP closure
//! *ascends*, unioning from one merged pair upward to the least closed partition
//! above it, and wants a disjoint-set forest. This harvest also deliberately
//! refuses to respect the accept partition — which is exactly what buys the
//! over-approximation.
//!
//! The filter *contract* is not claimed as ours: a compact over-approximating
//! automaton used as a sound reject stage in front of an exact verifier is
//! Luchaup, De Carli, Jha & Bach, *Deep packet inspection with DFA-trees and
//! parametrized language overapproximation*, INFOCOM 2014
//! (doi:10.1109/INFOCOM.2014.6847977), whose Definition 7 is `|D'| < |D|` with
//! `L(D) ⊆ L(D')` and which calls its shrunk DFAs "a special case of quotient
//! automaton"; restated as a cascade of crude over-approximating NFAs by Češka
//! et al., arXiv:1904.10786 (2019); and shipping as Hyperscan's
//! `HS_FLAG_PREFILTER`, whose matches are documented as a superset for an exact
//! matcher to confirm.

use alloc::{vec, vec::Vec};

use crate::projection::Projection;

/// Residency bound: a 16-lane `pshufb` / `vqtbl1q` holds one transition row, so a
/// quotient must partition into at most this many blocks to run register-resident
/// with no gather (Langdale, *Say Hello To My Little Friend*, 2018 — the
/// execution technique, applied here to an over-approximation rather than to an
/// exact small DFA).
pub const LANES: usize = 16;

/// How many quotients may be conjoined. Two independent shuffle chains issue in
/// parallel and their per-position conjunction costs a handful of throughput ops,
/// so the second is nearly free; a third adds a register chain to a filter that is
/// already at its selectivity floor.
pub const MAX_CONJUNCTS: usize = 2;

/// Union-find steps the whole harvest may spend before it stops looking. A partial
/// harvest is deterministic (pair order is fixed) and still sound — it just sees
/// fewer candidates.
const MAX_CLOSURE_STEPS: u64 = 1_500_000;

/// Distinct closed partitions kept before ranking.
const MAX_CANDIDATES: usize = 64;

/// A ≤16-state quotient, byte-expanded into exactly the form the shuffle kernel
/// consumes: `rows[b]` is the 16-lane transition row for byte `b`, indexed by the
/// current block.
///
/// Blocks are renumbered so the accepting ones are the top `blocks - threshold`,
/// which turns "did this quotient accept?" into one unsigned compare against a
/// splat rather than a second table lookup.
#[derive(Clone)]
pub struct Quotient {
    /// How many of the register's lanes this quotient actually uses; `blocks..LANES`
    /// are padding lanes the kernel never reaches.
    pub blocks: u8,
    /// Accepting iff `block >= threshold`.
    pub threshold: u8,
    /// The block the DFA's start state landed in after quotienting.
    pub start: u8,
    /// `rows[b][block]` — the destination block a run in `block` reaches on byte `b`.
    pub rows: [[u8; LANES]; 256],
}

/// Disjoint-set forest over compact state ids, with path halving.
struct Forest {
    parent: Vec<u16>,
}

impl Forest {
    fn new(n: usize) -> Self {
        let mut forest = Self { parent: vec![0; n] };
        forest.reset();
        forest
    }

    /// Every state its own root. `n <= MAX_CORE_STATES`, so the counter cannot
    /// outrun a `u16` and no cast is needed to say so.
    fn reset(&mut self) {
        for (p, i) in self.parent.iter_mut().zip(0u16..) {
            *p = i;
        }
    }

    fn find(&mut self, mut x: u16) -> u16 {
        while self.parent[usize::from(x)] != x {
            let grand = self.parent[usize::from(self.parent[usize::from(x)])];
            self.parent[usize::from(x)] = grand;
            x = grand;
        }
        x
    }

    fn join(&mut self, a: u16, b: u16) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        self.parent[usize::from(ra)] = rb;
        true
    }
}

/// The three buffers a pair closure would otherwise allocate afresh on every one of
/// the `O(n²)` pairs the sweep walks: the propagation stack, the canonical-id map,
/// and the partition being built. Each is rewritten per pair rather than reallocated,
/// so the whole sweep costs the allocations of one closure.
struct Scratch {
    work: Vec<(u16, u16)>,
    canon: Vec<u8>,
    block: Vec<u8>,
}

/// Harvest the lattice and return the chosen conjunction, most selective first.
/// An empty result means no closed partition small enough to hold in a register
/// carried any discriminating power.
pub fn harvest(core: &Projection) -> Vec<Quotient> {
    let mut forest = Forest::new(core.states);
    let mut scratch = Scratch {
        work: Vec::new(),
        canon: vec![u8::MAX; core.states],
        block: vec![0u8; core.states],
    };
    let mut steps = 0u64;
    let mut kept: Vec<(Vec<u8>, f32)> = Vec::new();

    // The core is capped at MAX_CORE_STATES, so the sweep counts in the id type
    // itself rather than casting a `usize` on every pair.
    let states = core.states as u16;
    'sweep: for p in 0..states {
        for q in (p + 1)..states {
            let Some(block) = close(core, &mut forest, &mut steps, &mut scratch, (p, q)) else {
                break 'sweep; // step budget spent; what we have is still sound
            };
            let blocks = block_count(block);
            if blocks <= 1 || usize::from(blocks) > LANES {
                continue;
            }
            // Cheapest refusal first: a partition already held is rejected on a byte
            // compare rather than after inducing a quotient nothing will keep.
            if kept.iter().any(|(seen, _)| seen.as_slice() == block) {
                continue;
            }
            let Some(induced) = induce(core, block, blocks) else {
                continue;
            };
            // Rank by how small a slice of its own state space accepts, ties to
            // the finer partition — the one closer to the truth it approximates.
            let accepting = f32::from(blocks - induced.threshold);
            kept.push((
                block.to_vec(),
                accepting / f32::from(blocks) - f32::from(blocks) * 1e-4,
            ));
            if kept.len() == MAX_CANDIDATES {
                break 'sweep;
            }
        }
    }

    select(core, kept)
}

/// Greedy by score, skipping any partition an already-chosen one already
/// distinguishes everything about — a conjunct that adds no discriminating power
/// is a second register chain bought for nothing.
fn select(core: &Projection, mut kept: Vec<(Vec<u8>, f32)>) -> Vec<Quotient> {
    kept.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut chosen: Vec<Vec<u8>> = Vec::new();
    let mut out: Vec<Quotient> = Vec::new();
    for (block, _) in kept {
        if out.len() == MAX_CONJUNCTS {
            break;
        }
        if chosen.iter().any(|c| refines(c, &block)) {
            continue;
        }
        let blocks = block_count(&block);
        if let Some(induced) = induce(core, &block, blocks) {
            out.push(induced.expand(core, blocks));
            chosen.push(block);
        }
    }
    out
}

/// The smallest closed partition identifying `pair`, as canonical block ids, or
/// `None` when the harvest's step budget ran out.
///
/// This is the classic pair-graph closure: merging two states forces their
/// successors to merge, transitively. Because the forest already carries
/// transitivity, propagating only the representative pair is complete.
fn close<'s>(
    core: &Projection,
    forest: &mut Forest,
    steps: &mut u64,
    scratch: &'s mut Scratch,
    pair: (u16, u16),
) -> Option<&'s [u8]> {
    forest.reset();
    scratch.work.clear();
    scratch.work.push(pair);
    while let Some((a, b)) = scratch.work.pop() {
        if !forest.join(a, b) {
            continue;
        }
        *steps += core.classes as u64;
        if *steps > MAX_CLOSURE_STEPS {
            return None;
        }
        for k in 0..core.classes {
            let (x, y) = (core.step(a, k), core.step(b, k));
            if forest.find(x) != forest.find(y) {
                scratch.work.push((x, y));
            }
        }
    }

    // Canonical block ids in first-seen order. The core is capped at
    // MAX_CORE_STATES (96), so a block id always fits a u8 and no truncation
    // check is needed here — a partition too coarse for a register is rejected by
    // the caller on block count, not smuggled through a clamp.
    scratch.canon.fill(u8::MAX);
    let mut blocks = 0u8;
    for (slot, i) in scratch.block.iter_mut().zip(0u16..) {
        let root = usize::from(forest.find(i));
        if scratch.canon[root] == u8::MAX {
            scratch.canon[root] = blocks;
            blocks += 1;
        }
        *slot = scratch.canon[root];
    }
    Some(&scratch.block)
}

/// Does `fine` already distinguish everything `coarse` does — i.e. does adding
/// `coarse` to a conjunction that holds `fine` buy no discriminating power?
fn refines(fine: &[u8], coarse: &[u8]) -> bool {
    let mut seen = [i16::MIN; LANES];
    for (&a, &b) in fine.iter().zip(coarse) {
        let slot = &mut seen[usize::from(a)];
        if *slot == i16::MIN {
            *slot = i16::from(b);
        } else if *slot != i16::from(b) {
            return false;
        }
    }
    true
}

fn block_count(block: &[u8]) -> u8 {
    block.iter().copied().max().unwrap_or(0).saturating_add(1)
}

/// A closed partition that survived [`induce`], at the resolution the check itself
/// runs at — one column per byte *class* rather than per byte.
///
/// Split from the byte-expanded [`Quotient`] because the sweep asks a great many
/// candidates for their [`Induced::threshold`] and keeps at most
/// [`MAX_CONJUNCTS`] of them; expanding the other rows to their kernel form would be
/// four kilobytes written per candidate to be dropped on the next line.
struct Induced {
    /// Accepting iff `block >= threshold`, over the renumbered blocks.
    threshold: u8,
    /// The block the DFA's start state landed in, renumbered.
    start: u8,
    /// `table[block][class]` — the successor the closure check re-derived and agreed
    /// with. [`Induced::expand`] is what turns a class back into 256 bytes.
    table: [[u8; 256]; LANES],
}

impl Induced {
    /// Widen the class columns back out to the byte rows the shuffle kernel indexes.
    fn expand(&self, core: &Projection, blocks: u8) -> Quotient {
        // Lanes past the block count keep their zero: a state that does not exist is
        // never entered, so its column is never read.
        let mut rows = [[0u8; LANES]; 256];
        for (row, &k) in rows.iter_mut().zip(&core.class_of) {
            for (lane, state) in row.iter_mut().zip(&self.table[..usize::from(blocks)]) {
                *lane = state[usize::from(k)];
            }
        }
        Quotient {
            blocks,
            threshold: self.threshold,
            start: self.start,
            rows,
        }
    }
}

/// Check the quotient a closed partition induces, or decline.
///
/// The harvest's arithmetic is **not trusted**: the transition is re-derived from
/// the raw partition and any disagreement rejects the candidate, because a
/// partition that is not actually closed over-approximates nothing. The other two
/// declinatures are economic — a quotient whose start block accepts would accept
/// at every position, and one where every block accepts would reject nothing.
fn induce(core: &Projection, block: &[u8], blocks: u8) -> Option<Induced> {
    if usize::from(blocks) > LANES {
        return None;
    }
    let mut block_accepts = [false; LANES];
    for (&acc, &b) in core.accept.iter().zip(block) {
        if acc {
            block_accepts[usize::from(b)] = true;
        }
    }
    let nb = usize::from(blocks);
    // A count over at most LANES slots fits a u8.
    let threshold = block_accepts[..nb].iter().filter(|&&a| !a).count() as u8;
    if threshold == 0 || threshold == blocks {
        return None; // rejects nothing / accepts nothing worth running
    }

    // Renumber so non-accepting blocks come first: `state >= threshold` is then
    // the whole accept test.
    let mut relabel = [0u8; LANES];
    let (mut lo, mut hi) = (0u8, threshold);
    for (slot, &acc) in relabel.iter_mut().zip(&block_accepts[..nb]) {
        let next = if acc { &mut hi } else { &mut lo };
        *slot = *next;
        *next += 1;
    }
    let start = relabel[usize::from(block[usize::from(core.start)])];
    if start >= threshold {
        return None; // the start block accepts, so every position would
    }

    let mut table = [[0u8; 256]; LANES];
    let mut filled = [false; LANES];
    for (&b, i) in block.iter().zip(0u16..) {
        let from = usize::from(relabel[usize::from(b)]);
        for k in 0..core.classes {
            let to = relabel[usize::from(block[usize::from(core.step(i, k))])];
            if filled[from] {
                if table[from][k] != to {
                    return None; // not closed after all
                }
            } else {
                table[from][k] = to;
            }
        }
        filled[from] = true;
    }
    if filled[..nb].iter().any(|&f| !f) {
        return None; // an unreachable block means the renumbering lied
    }

    Some(Induced {
        threshold,
        start,
        table,
    })
}
