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
//! # The memoryless priors are kept, deliberately
//!
//! [`Prior::Uniform`] and [`Prior::Text`] are the memoryless special cases: every
//! row of their matrix is the same marginal, so the chain reduces to independent
//! draws exactly. They stay because the gate decides on the **worst case over all
//! priors**, and because keeping the superseded model addressable is what lets its
//! error be measured rather than asserted.

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

/// The measured first-order chain over real source bytes.
///
/// Minted on Darwin 25.5.0 arm64, 2026-07-29, over 6,418 source files (64.1 MiB) of
/// this repository — Rust, Zig, Go, Python, TypeScript, SQL, Swift, Markdown, TOML.
/// Re-mint with `cargo run --release --example mint` from the repository root.
///
/// The diagonal is the reason this type exists. Each class's chance of repeating,
/// against its marginal share:
///
/// | class | marginal | repeats | ratio |
/// |---|---|---|---|
/// | `Space` | 0.1817 | 0.4517 | 2.5x |
/// | `Break` | 0.0271 | 0.1048 | 3.9x |
/// | `Lower` | 0.5703 | 0.7683 | 1.3x |
/// | `Upper` | 0.0560 | 0.3565 | 6.4x |
/// | `Digit` | 0.0186 | 0.3863 | **20.8x** |
/// | `Punct` | 0.1325 | 0.2524 | 1.9x |
/// | `High`  | 0.0139 | 0.9167 | **66.0x** |
///
/// A memoryless prior therefore under-prices a `k`-byte digit run by about `20.8^k`
/// — which is precisely the error that let a filter rejecting essentially nothing
/// look like one rejecting everything.
///
/// `Space` never reaching `Break` is real rather than a rounding artifact: this tree
/// is linted, so trailing whitespace before a newline is effectively absent.
// A transition matrix is one table, and reading it by row against the header below
// is the whole point — so the row-per-line layout is pinned rather than reflowed.
#[rustfmt::skip]
pub const SOURCE: Chain = Chain {
    //     Space     Break     Lower     Upper     Digit     Punct      High
    next: [
        [0.451677, 0.000000, 0.333127, 0.042422, 0.016962, 0.149964, 0.005848], // Space
        [0.713848, 0.104793, 0.063638, 0.016030, 0.000283, 0.101210, 0.000199], // Break
        [0.082433, 0.008042, 0.768254, 0.030242, 0.009533, 0.101450, 0.000046], // Lower
        [0.042460, 0.007471, 0.514333, 0.356499, 0.002727, 0.076414, 0.000095], // Upper
        [0.110313, 0.037564, 0.075528, 0.023526, 0.386326, 0.365925, 0.000818], // Digit
        [0.211487, 0.139038, 0.299459, 0.076937, 0.020348, 0.252417, 0.000314], // Punct
        [0.066678, 0.008770, 0.001777, 0.000535, 0.001748, 0.003810, 0.916681], // High
    ],
    start: [0.181717, 0.027072, 0.570280, 0.055983, 0.018575, 0.132492, 0.013881],
};

/// Marginal frequency of every byte value over the same minted corpus as [`SOURCE`].
///
/// **Per-byte, and that resolution is load-bearing.** The class chain carries how
/// bytes *cluster*; this carries how often each one occurs, and the two answer
/// different questions. Pricing an engine's escape set at class resolution treats `a`
/// and `f` as equally common when `a` occurs about three times as often — which is
/// exactly the difference between a pattern whose accelerator trips constantly (worth
/// fronting) and one whose accelerator earns its keep (not worth fronting). Arming on
/// the class average did both wrong at once.
///
/// Minted alongside [`SOURCE`] on the same 64.1 MiB of real source. The evidence that
/// the resolution matters is in the excursion solver's own spread: read at class
/// resolution, the eleven lead bytes it inverts disagreed by 10x (3.6 to 35.2); read
/// from this table they agree within 1.7x (7.06 to 11.78). The variance was the
/// approximation, not the measurement.
pub const SOURCE_BYTES: [f64; 256] = [
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.02348194, 0.02707233, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.15823466, 0.00093228, 0.01212123, 0.00084917, 0.00016688, 0.00035293, 0.00058606, 0.00179882,
    0.01068197, 0.01038673, 0.00162193, 0.00150419, 0.01070865, 0.00637019, 0.01385492, 0.00496256,
    0.00523016, 0.00400453, 0.00219353, 0.00122989, 0.00159107, 0.00078858, 0.00147180, 0.00072862,
    0.00097900, 0.00035768, 0.00707436, 0.00110169, 0.00086292, 0.00846172, 0.00127486, 0.00011155,
    0.00020109, 0.00435993, 0.00116575, 0.00333640, 0.00214529, 0.00586268, 0.00200241, 0.00110330,
    0.00083414, 0.00317845, 0.00018502, 0.00039273, 0.00256395, 0.00203484, 0.00377334, 0.00244100,
    0.00207299, 0.00029789, 0.00431163, 0.00501794, 0.00464331, 0.00189854, 0.00103260, 0.00053590,
    0.00033695, 0.00035935, 0.00009711, 0.00275525, 0.00427699, 0.00275255, 0.00001399, 0.01390198,
    0.00442084, 0.03637424, 0.00817733, 0.02109737, 0.02265156, 0.07370121, 0.01314766, 0.01015872,
    0.01218381, 0.04055988, 0.00110193, 0.00451404, 0.02458769, 0.01465999, 0.04106374, 0.03908288,
    0.01906260, 0.00161610, 0.04674633, 0.03927468, 0.05454883, 0.01670062, 0.00695385, 0.00526917,
    0.00823928, 0.00747249, 0.00133413, 0.00376073, 0.00083514, 0.00375906, 0.00002829, 0.00000000,
    0.00421794, 0.00000110, 0.00001260, 0.00000134, 0.00000124, 0.00000061, 0.00015301, 0.00000790,
    0.00001624, 0.00002082, 0.00000115, 0.00000051, 0.00000202, 0.00000034, 0.00000030, 0.00000021,
    0.00012277, 0.00000094, 0.00015467, 0.00000512, 0.00420130, 0.00011869, 0.00000476, 0.00001790,
    0.00000150, 0.00000086, 0.00000103, 0.00000013, 0.00000583, 0.00000068, 0.00000024, 0.00000232,
    0.00000219, 0.00000112, 0.00000693, 0.00000202, 0.00000609, 0.00000750, 0.00002823, 0.00001946,
    0.00000007, 0.00000179, 0.00000098, 0.00000192, 0.00000080, 0.00000022, 0.00000025, 0.00000016,
    0.00000077, 0.00000482, 0.00000504, 0.00000228, 0.00000031, 0.00000454, 0.00000109, 0.00003290,
    0.00000122, 0.00000089, 0.00000067, 0.00000204, 0.00000256, 0.00000174, 0.00000012, 0.00000018,
    0.00000000, 0.00000000, 0.00006262, 0.00002132, 0.00000003, 0.00000016, 0.00000000, 0.00000001,
    0.00000000, 0.00000305, 0.00000147, 0.00000147, 0.00000119, 0.00000000, 0.00001351, 0.00000446,
    0.00000129, 0.00000048, 0.00000001, 0.00000000, 0.00000009, 0.00000000, 0.00000000, 0.00000000,
    0.00000024, 0.00000055, 0.00000000, 0.00000006, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000001, 0.00000106, 0.00454794, 0.00000024, 0.00000058, 0.00000043, 0.00000042, 0.00000010,
    0.00000021, 0.00000003, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000021,
    0.00000083, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
];

/// Which byte model the gate is reasoning under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prior {
    /// Uniform random bytes, drawn independently.
    Uniform,
    /// Source-text class shares, drawn independently. The memoryless model the
    /// persistence matrix supersedes.
    Text,
    /// Source text as a first-order chain — the only one of the three that prices
    /// a class run correctly.
    Source,
}

/// The chains the gate sweeps when a caller names none — every [`Prior`], resolved so
/// the list cannot drift from the enum.
///
/// A caller whose documents are not source text (English prose, JSON logs, minified
/// JavaScript, DNA) should mint their own and pass them in a [`crate::Policy`]: these
/// three describe a code tree, and a prior is a claim about the bytes that will
/// actually be searched. `cargo run --release --example mint` prints the matrix.
pub const DEFAULT_CHAINS: [Chain; Prior::ALL.len()] = [
    Prior::Uniform.chain(),
    Prior::Text.chain(),
    Prior::Source.chain(),
];

impl Prior {
    /// Every prior the gate consults. The decision is the **worst case** over this
    /// set, so adding one can only tighten the gate.
    pub const ALL: [Self; 3] = [Self::Uniform, Self::Text, Self::Source];

    /// The first-order (block, class) chain this prior resolves to.
    #[must_use]
    pub const fn chain(self) -> Chain {
        match self {
            Self::Uniform => UNIFORM,
            Self::Text => TEXT,
            Self::Source => SOURCE,
        }
    }

    /// Marginal frequency of each byte value under this prior — what the escape-set
    /// model reads. Only [`Prior::Source`] has per-byte resolution; the memoryless
    /// priors spread each class's mass evenly across its members, which is all the
    /// structure they have.
    #[must_use]
    pub fn byte_freq(self) -> [f64; 256] {
        match self {
            Self::Source => SOURCE_BYTES,
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
}
