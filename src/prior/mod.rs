//! What a byte is likely to be, given the byte before it.
//!
//! The selectivity gate needs to predict how often a quotient accepts without
//! looking at the document it is about to filter. That prediction is only as good
//! as its model of the bytes, and the obvious model is wrong in a way that matters
//! enormously.
//!
//! # Why a memoryless prior cannot price a run
//!
//! Draw every position independently and a `k`-byte class requirement costs `p^k`.
//! Real text does not work that way: classes cluster, so a digit is far likelier to
//! follow a digit than its marginal share suggests. Measured over the source tree
//! ([`SOURCE`]), the diagonal of the transition matrix runs several times each
//! class's marginal — which means a memoryless model under-counts a `k`-run by
//! roughly that ratio to the `k`. At one byte the error is noise; by forty bytes it
//! is astronomical, and a gate fed that estimate arms a filter that rejects
//! essentially nothing while believing it rejects everything.
//!
//! So the model here is a **first-order chain over byte classes**: seven classes,
//! a 7x7 transition matrix, and a uniform spread within each class. That is enough
//! structure to price a run correctly and little enough to stay a compile-time
//! constant — nothing observes traffic, learns, or adapts at runtime.
//!
//! # One model, four corpora
//!
//! The shape above is the model; [`minted`] holds what it measured. Four corpora are
//! shipped — a polyglot code tree, English literary prose, machine-generated JSON, and
//! sixteen systems' production logs — because a chain minted on source text is a claim
//! about source text, and shipping only that one meant a caller filtering prose was
//! being priced under a model of somebody else's Rust. They disagree at the coarsest
//! level: `Space` is the most self-following class in a code tree and the *least* in
//! prose. Read [`minted`] for what each says and where each came from.
//!
//! # The memoryless priors are kept, deliberately
//!
//! [`Prior::Uniform`] and [`Prior::Text`] are the memoryless special cases: every
//! row of their matrix is the same marginal, so the chain reduces to independent
//! draws exactly. They stay because the gate decides on the **worst case over all
//! priors**, and because keeping the superseded model addressable is what lets its
//! error be measured rather than asserted.

mod minted;

pub use minted::{JSON, JSON_BYTES, LOG, LOG_BYTES, PROSE, PROSE_BYTES, SOURCE, SOURCE_BYTES};

/// Byte classes coarse enough that a 7x7 matrix over a real corpus has dense
/// support, and fine enough that the runs patterns actually require — digits,
/// letters, whitespace — are each their own row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Horizontal whitespace. The most persistent class in indented source.
    Space,
    /// Line breaks, split from `Space` because a great many patterns are
    /// line-bounded and `\n` is what ends them.
    Break,
    /// `a`..=`z`.
    Lower,
    /// `A`..=`Z`.
    Upper,
    /// `0`..=`9`.
    Digit,
    /// Printable ASCII punctuation.
    Punct,
    /// Non-ASCII and the remaining control bytes.
    High,
}

/// How many rows/columns [`Class`]'s transition matrices carry.
pub const CLASSES: usize = 7;

impl Class {
    /// Index order for every matrix in this module.
    pub const ALL: [Self; CLASSES] = [
        Self::Space,
        Self::Break,
        Self::Lower,
        Self::Upper,
        Self::Digit,
        Self::Punct,
        Self::High,
    ];

    /// Which class a raw byte falls into.
    #[must_use]
    pub const fn of(b: u8) -> Self {
        match b {
            b' ' | b'\t' => Self::Space,
            b'\n' | b'\r' => Self::Break,
            b'a'..=b'z' => Self::Lower,
            b'A'..=b'Z' => Self::Upper,
            b'0'..=b'9' => Self::Digit,
            0x21..=0x2F | 0x3A..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E => Self::Punct,
            _ => Self::High,
        }
    }
}

/// How many byte values each class holds, counted rather than tabulated so the
/// spread inside a class can never disagree with [`Class::of`].
const MEMBERS: [f64; CLASSES] = {
    let mut n = [0.0f64; CLASSES];
    let mut b = 0u8;
    // Counted straight into f64 — every tally is ≤ 256, so it is exact — and walked
    // to `u8::MAX` inclusive, which is why the break sits mid-loop.
    loop {
        n[Class::of(b) as usize] += 1.0;
        if b == u8::MAX {
            break n;
        }
        b += 1;
    }
};

/// How many of the 256 byte values `class` holds. Counted from [`Class::of`], so
/// the spread inside a class can never disagree with the classifier.
#[must_use]
pub const fn members(class: Class) -> f64 {
    MEMBERS[class as usize]
}

/// A first-order Markov chain over [`Class`].
#[derive(Debug, Clone, Copy)]
pub struct Chain {
    /// `next[i][j]` is the probability that a byte of class `j` follows one of
    /// class `i`. Rows sum to 1.
    pub next: [[f64; CLASSES]; CLASSES],
    /// The marginal class distribution — where a scan is assumed to begin.
    pub start: [f64; CLASSES],
}

impl Chain {
    /// The concrete byte distribution following a byte of class `from`: the class
    /// probability spread uniformly across that class's members.
    #[must_use]
    pub fn bytes_after(&self, from: Class) -> [f64; 256] {
        let row = self.next[from as usize];
        let mut w = [0.0f64; 256];
        for (slot, b) in w.iter_mut().zip(0..=u8::MAX) {
            let class = Class::of(b) as usize;
            *slot = row[class] / MEMBERS[class];
        }
        w
    }

    /// A memoryless chain: every row is the same marginal, so successive draws are
    /// independent and a `k`-run is priced `p^k`. The shape the persistence matrix
    /// replaces, retained so the difference is measurable.
    #[must_use]
    pub const fn memoryless(marginal: [f64; CLASSES]) -> Self {
        Self {
            next: [marginal; CLASSES],
            start: marginal,
        }
    }

    /// [`Chain::memoryless`] read back: the one row every row is, or `None` when this
    /// chain has persistence to speak of.
    ///
    /// Asked by [`crate::worst_case`], where it is worth a scan of six rows to learn:
    /// a chain that draws the next class the same way whatever the last one was cannot
    /// have the last one steer the draw, so its joint step collapses from a 7x7
    /// product to one total and one scaling. That is the same collapse
    /// [`crate::prior`] describes above, taken rather than merely noted.
    pub(crate) fn marginal(&self) -> Option<&[f64; CLASSES]> {
        let (first, rest) = self.next.split_first()?;
        rest.iter().all(|row| row == first).then_some(first)
    }
}

/// Every byte value equally likely. Assumes nothing about the document at all.
const UNIFORM: Chain = Chain::memoryless({
    // Each class's share is just its size out of 256, so the member counts are the
    // table already — divided in place rather than transcribed.
    let mut m = MEMBERS;
    let mut i = 0;
    while i < CLASSES {
        m[i] /= 256.0;
        i += 1;
    }
    m
});

/// The memoryless source-text marginal — coarse class shares from a source tree,
/// drawn independently.
const TEXT: Chain = Chain::memoryless([0.222, 0.030, 0.487, 0.055, 0.015, 0.191, 0.0001]);

/// Which byte model the gate is reasoning under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prior {
    /// Uniform random bytes, drawn independently.
    Uniform,
    /// Source-text class shares, drawn independently. The memoryless model the
    /// persistence matrix supersedes.
    Text,
    /// Source text as a first-order chain — the first of these that prices a class
    /// run correctly. See [`SOURCE`].
    Source,
    /// English literary prose. See [`PROSE`], which disagrees with [`SOURCE`] about
    /// whether a space is likely to be followed by another one.
    Prose,
    /// Machine-generated JSON, where every class persists and none is rare. See
    /// [`JSON`].
    Json,
    /// Production service logs from sixteen emitters. See [`LOG`].
    Log,
}

/// The chains the gate sweeps when a caller names none — every [`Prior`], resolved so
/// the list cannot drift from the enum.
///
/// Four measured corpora and two memoryless models, and the breadth is the point: the
/// gate takes the **worst case** over this set, so sweeping it is what lets a default
/// [`Policy`](crate::Policy) be safe for a caller who never says what they are
/// searching. A caller who *does* know narrows it — `chains: &[Prior::Json.chain()]`
/// — and gets a better-informed and therefore looser decision. Narrowing is the only
/// direction that loosens; adding a corpus can only tighten.
///
/// A corpus none of these describes (minified JavaScript, DNA, a wire protocol) is
/// still worth minting: `cargo run --release --example mint -- mine` prints the matrix
/// and the byte table for whatever `$SHENG_CORPUS` and `$SHENG_KINDS` point at.
pub const DEFAULT_CHAINS: [Chain; Prior::ALL.len()] = {
    // Resolved by walking `ALL` rather than transcribed, so "cannot drift from the
    // enum" is a property of this constant instead of a request to whoever adds one.
    let mut out = [UNIFORM; Prior::ALL.len()];
    let mut i = 0;
    while i < out.len() {
        out[i] = Prior::ALL[i].chain();
        i += 1;
    }
    out
};

impl Prior {
    /// Every prior the gate consults. The decision is the **worst case** over this
    /// set, so adding one can only tighten the gate.
    pub const ALL: [Self; 6] = [
        Self::Uniform,
        Self::Text,
        Self::Source,
        Self::Prose,
        Self::Json,
        Self::Log,
    ];

    /// The first-order (block, class) chain this prior resolves to.
    #[must_use]
    pub const fn chain(self) -> Chain {
        match self {
            Self::Uniform => UNIFORM,
            Self::Text => TEXT,
            Self::Source => SOURCE,
            Self::Prose => PROSE,
            Self::Json => JSON,
            Self::Log => LOG,
        }
    }

    /// Marginal frequency of each byte value under this prior — what the escape-set
    /// model reads. Every *measured* prior has per-byte resolution; the two memoryless
    /// ones spread each class's mass evenly across its members, which is all the
    /// structure they have.
    #[must_use]
    pub fn byte_freq(self) -> [f64; 256] {
        match self {
            Self::Source => SOURCE_BYTES,
            Self::Prose => PROSE_BYTES,
            Self::Json => JSON_BYTES,
            Self::Log => LOG_BYTES,
            other => {
                let chain = other.chain();
                let mut out = [0.0f64; 256];
                for (slot, b) in out.iter_mut().zip(0..=u8::MAX) {
                    let class = Class::of(b);
                    *slot = chain.start[class as usize] / members(class);
                }
                out
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_holds_at_least_one_byte_and_they_partition_256() {
        assert_eq!(MEMBERS.iter().sum::<f64>(), 256.0);
        assert!(MEMBERS.iter().all(|&n| n > 0.0));
    }

    #[test]
    fn every_row_of_every_chain_is_a_distribution() {
        for prior in Prior::ALL {
            let chain = prior.chain();
            for (i, row) in chain.next.iter().enumerate() {
                let sum: f64 = row.iter().sum();
                assert!((sum - 1.0).abs() < 1e-3, "{prior:?} row {i} sums to {sum}");
            }
            let start: f64 = chain.start.iter().sum();
            assert!(
                (start - 1.0).abs() < 1e-3,
                "{prior:?} start sums to {start}"
            );
            for class in Class::ALL {
                let bytes: f64 = chain.bytes_after(class).iter().sum();
                assert!(
                    (bytes - 1.0).abs() < 1e-3,
                    "{prior:?} after {class:?}: {bytes}"
                );
            }
        }
    }

    /// The whole reason this module exists: under the measured chain a class is
    /// markedly likelier to repeat than its marginal share, and a memoryless prior
    /// has no way to express that.
    #[test]
    fn the_measured_chain_is_persistent_where_the_memoryless_one_is_not() {
        for class in Class::ALL {
            let i = class as usize;
            let marginal = SOURCE.start[i];
            let repeat = SOURCE.next[i][i];
            assert!(
                repeat > marginal,
                "{class:?} must be likelier to repeat than to occur: {repeat} vs {marginal}"
            );
            assert!(
                (TEXT.next[i][i] - TEXT.start[i]).abs() < 1e-9,
                "a memoryless prior is one with no persistence to speak of"
            );
        }
    }

    /// The classes patterns actually build runs out of are the ones a memoryless
    /// prior misprices worst, because their marginal share is small and their
    /// clustering is extreme. `Lower` is only 1.3x because it already dominates
    /// source text; a digit is 20x and a non-ASCII byte 66x.
    #[test]
    fn the_sparse_classes_are_the_ones_a_memoryless_prior_misprices() {
        for class in [Class::Digit, Class::Upper, Class::High] {
            let i = class as usize;
            let ratio = SOURCE.next[i][i] / SOURCE.start[i];
            assert!(ratio > 6.0, "{class:?} persistence ratio only {ratio}");
        }
    }

    /// Four measured corpora are only worth four sets of constants if they say four
    /// different things — otherwise the sweep costs the gate breadth it does not buy.
    ///
    /// The clearest disagreement is whitespace. Source text indents, so `Space` is the
    /// likeliest class to follow itself in the whole tree; prose puts one space between
    /// words, so there the same class is the *least* likely to repeat of any measured
    /// row. That is a sign flip in the model's coarsest term, not a shift in a
    /// decimal — a `[ ]{2,}` run is nearly certain under one chain and nearly
    /// impossible under another. It also catches the paste error that would otherwise
    /// be invisible: a table copied into two constants.
    #[test]
    fn the_measured_corpora_disagree_about_the_bytes_they_measured() {
        let space = Class::Space as usize;
        assert!(
            SOURCE.next[space][space] > SOURCE.start[space],
            "indented source has to make a space likely to follow a space"
        );
        assert!(
            PROSE.next[space][space] < PROSE.start[space] / 4.0,
            "prose separates words with one space, so its Space row cannot persist"
        );
        for (name, chain, freq) in [
            ("SOURCE", SOURCE, SOURCE_BYTES),
            ("PROSE", PROSE, PROSE_BYTES),
            ("JSON", JSON, JSON_BYTES),
            ("LOG", LOG, LOG_BYTES),
        ] {
            for (other, rival, rival_freq) in [
                ("SOURCE", SOURCE, SOURCE_BYTES),
                ("PROSE", PROSE, PROSE_BYTES),
                ("JSON", JSON, JSON_BYTES),
                ("LOG", LOG, LOG_BYTES),
            ] {
                if name == other {
                    continue;
                }
                assert_ne!(
                    chain.start, rival.start,
                    "{name} and {other} carry one corpus between them"
                );
                assert_ne!(
                    freq, rival_freq,
                    "{name}_BYTES and {other}_BYTES are one table"
                );
            }
        }
    }
}
