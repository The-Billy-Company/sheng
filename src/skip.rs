//! Finding the next byte that matters, without looking at the ones that do not.
//!
//! [`crate::shuffle`] is bound by its **load ports**, not by its instruction count:
//! every byte costs a haystack load and a transition-row load, and that pair sits
//! at the machine's load-port ceiling on current silicon. Deleting arithmetic from
//! that loop buys nothing (the `max` experiment measured inside the noise). The
//! only lever left is to stop reading bytes.
//!
//! # The block that holds still
//!
//! An unanchored pattern's DFA has a start state whose `.*` prefix loops on almost
//! every byte, and quotienting preserves that: writing `B₀` for the block holding the
//! real start state `q₀`, if `δ(q₀,b) = q₀` then `block(δ(q₀,b)) = B₀`, and the
//! substitution property makes `δ_q(B₀,b)` well defined from any member — so
//!
//! > every self-loop of the real start state is a self-loop of `B₀`.
//!
//! The quotient's start block is therefore **at least as sticky** as the state the
//! engine accelerates, and `Escape(B₀) ⊆ Escape(q₀)`. While the run sits in a
//! non-accepting block, consuming self-loop bytes cannot change the state and cannot
//! visit an accepting block, so those bytes are provably uninformative and skipping
//! them is exact rather than merely sound.
//!
//! # Why this is not just the engine's accelerator again
//!
//! `regex-automata` accelerates a state only when it escapes on **at most three**
//! bytes, because `memchr3` is where its instrument stops. The interesting band is
//! above that line and it is wide: digit-and-hyphen patterns and identifier-shaped
//! ones hold `B₀` for most of a typical document and escape on far more than three
//! bytes — and the engine accelerates neither. Below the line the sieve gains
//! nothing it can sell, since a rival already `memchr`-ing the identical byte
//! cannot be beaten by a filter that has to find the same byte first. So the width
//! that pays is exactly the width the engine declines, which is a happier
//! arrangement than it sounds: the two instruments partition the space instead of
//! racing.
//!
//! # Instruments
//!
//! * **1..=3 bytes** — [`memchr`]. Its searcher is better than anything worth
//!   hand-rolling at that width, and the crate is already in the dependency graph
//!   under `regex-automata`.
//! * **4..=128 bytes, all ASCII** — a nibble classifier: `lo[b & 0xF] & hi[b >> 4]`
//!   is nonzero exactly on set members, which is two table lookups and an `and` for
//!   sixteen bytes at a time. This is Hyperscan's **shufti** (Langdale & Gough,
//!   *Hyperscan: A Fast Multi-pattern Regex Matcher for Modern CPUs*, NSDI 2019) and
//!   the same construction simdjson uses for structural character classification.
//! * **anything else** — no skip. A set with a non-ASCII member needs a second
//!   table pair to stay exact, and declining costs only the speedup.
//!
//! There is no third instrument, and that is a measured decision rather than a gap. The
//! obvious repair for [`Instrument::Wide`]'s per-block narrowing on NEON is a *carried
//! mask* over four blocks at once, and it was priced across escape densities from
//! nothing to one byte in ten: its best case does not clear
//! [`MARGIN`](crate::price::MARGIN), and through the band a wide set really occupies it
//! loses by up to a factor of two, because the shipped loop stops one or two blocks in
//! where the carried one has already paid for four. A wider stride is the wrong trade
//! for a loop whose whole purpose is to stop early.
//!
//! Every instrument is checked against [`Escape::find_scalar`], the obvious
//! `set.contains(b)` loop, over every byte value and every set shape the harvest can
//! produce. A skip that overshoots one escape byte would make the sieve reject a
//! document that matches, which is the one bug in this crate that is not survivable.

use alloc::vec::Vec;

/// Widest escape set the classifier will accept. Above this the set is closer to
/// "most bytes" than to "a class", the runs between hits are too short for a
/// vector probe to amortize, and the composition kernel is the better answer
/// anyway.
const MAX_WIDE: usize = 128;

/// How to find the next byte that leaves a block, and which block that is.
pub struct Skip {
    /// The block this is a skip *for*. Only valid while the run is here.
    pub resident: u8,
    escape: Escape,
    leaves: Vec<u8>,
}

/// Which searcher a skip runs, and therefore what its excursions cost.
///
/// Named in the calibration rather than inferred, because the two restart at
/// genuinely different prices: `memchr` re-enters an aligned multi-stage loop, the
/// classifier re-enters two resident registers and a sixteen-byte step. Indexes
/// `Calibration::skip_excursion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instrument {
    /// `memchr` over one to three byte values.
    Few = 0,
    /// The nibble classifier over a wider ASCII set.
    Wide = 1,
}

/// The instrument, chosen by how many byte values leave the block.
enum Escape {
    /// Nothing leaves: the block is an absorbing non-accepting sink, and a run that
    /// reaches it has already decided the whole rest of the document.
    Never,
    /// One to three values, searched by [`memchr`].
    Few([u8; 3], usize),
    /// A wider ASCII set, searched by the nibble classifier.
    Wide { lo: [u8; 16], hi: [u8; 16] },
}

impl Skip {
    /// The skip for `block` in a quotient whose rows are `rows`, or `None` when
    /// there is nothing here worth skipping past.
    ///
    /// Declines a block that escapes on everything (no run to skip) and one whose
    /// escape set the classifier cannot represent exactly. Both refusals are
    /// answers, not failures: the caller keeps the composition kernel.
    #[must_use]
    pub fn of(rows: &[[u8; 16]; 256], block: u8) -> Option<Self> {
        let mut escape = Vec::new();
        for b in 0..=255u8 {
            if rows[usize::from(b)][usize::from(block)] != block {
                escape.push(b);
            }
        }
        Some(Self {
            resident: block,
            escape: Escape::of(&escape)?,
            leaves: escape,
        })
    }

    /// The byte values that leave [`Skip::resident`].
    ///
    /// Exposed because this skip loop **is** an accelerated DFA, so the arithmetic
    /// that already prices the engine's accelerator prices this one too —
    /// `Calibration::rival_per_byte(skip.leaves(), freq)`, with no second cost model
    /// to keep honest. The one place the two differ is that a wide set is searched
    /// by a nibble classifier whose excursions are cheaper than a full DFA's, so
    /// the shared arithmetic **over**-prices this skip. Erring toward declining a
    /// skip that would have paid is the same direction every other refusal in this
    /// crate takes.
    #[must_use]
    pub fn leaves(&self) -> &[u8] {
        &self.leaves
    }

    /// Which searcher this skip runs. A block nothing leaves reads as
    /// [`Instrument::Few`], which costs nothing either way — its price is the
    /// special case in `Calibration::skip_per_byte`, not a blend.
    #[must_use]
    pub fn instrument(&self) -> Instrument {
        match self.escape {
            Escape::Wide { .. } => Instrument::Wide,
            Escape::Never | Escape::Few(..) => Instrument::Few,
        }
    }

    /// Offset of the first byte of `hay` that leaves [`Skip::resident`], or `None`
    /// when the whole slice keeps the run where it is.
    ///
    /// `None` is the load-bearing answer: it says a caller sitting in a
    /// non-accepting block may retire every remaining byte without reading them.
    #[must_use]
    pub fn find(&self, hay: &[u8]) -> Option<usize> {
        match &self.escape {
            Escape::Never => None,
            Escape::Few(set, 1) => memchr::memchr(set[0], hay),
            Escape::Few(set, 2) => memchr::memchr2(set[0], set[1], hay),
            Escape::Few(set, _) => memchr::memchr3(set[0], set[1], set[2], hay),
            Escape::Wide { lo, hi } => wide::find(lo, hi, hay),
        }
    }

    /// The definition, kept in the shipping build because it is what [`Skip::find`]
    /// is tested against and what a target with no byte shuffle runs.
    #[must_use]
    pub fn find_scalar(&self, hay: &[u8]) -> Option<usize> {
        match &self.escape {
            Escape::Never => None,
            Escape::Few(set, n) => hay.iter().position(|b| set[..*n].contains(b)),
            Escape::Wide { lo, hi } => hay
                .iter()
                .position(|&b| lo[usize::from(b & 0xF)] & hi[usize::from(b >> 4)] != 0),
        }
    }
}

impl Escape {
    fn of(escape: &[u8]) -> Option<Self> {
        match *escape {
            [] => Some(Self::Never),
            [a] => Some(Self::Few([a, a, a], 1)),
            [a, b] => Some(Self::Few([a, b, b], 2)),
            [a, b, c] => Some(Self::Few([a, b, c], 3)),
            // A high byte would need a second table pair to stay exact, and the
            // whole point of this module is that it never guesses.
            _ if escape.len() > MAX_WIDE || escape.iter().any(|&b| b >= 0x80) => None,
            _ => {
                // `lo[l]` carries one bit per high nibble, `hi[h]` selects it. The
                // product is nonzero exactly for members — and for `b >= 0x80` the
                // high nibble is 8..=15, where `hi` is zero, which is why the set
                // has to be ASCII for this to be the truth rather than a guess.
                let mut lo = [0u8; 16];
                let hi: [u8; 16] = core::array::from_fn(|h| if h < 8 { 1 << h } else { 0 });
                for &b in escape {
                    lo[usize::from(b & 0xF)] |= 1 << (b >> 4);
                }
                Some(Self::Wide { lo, hi })
            },
        }
    }
}

/// The nibble classifier, one implementation per byte-shuffle instruction set.
///
/// Structured so every vector path and the fallback share one entry point and one
/// meaning: `find` returns the first offset whose byte is in the set, and the tests
/// hold all of them to the same answer on the same bytes. The vector bodies
/// themselves live in [`crate::arch`], one file per instruction set, alongside
/// [`crate::shuffle`]'s composition kernel.
pub(crate) mod wide {
    use crate::arch;
    // Named only by the vector arms below, so on a target with no byte shuffle there
    // is no variant left to match and nothing to import.
    #[cfg(any(
        target_arch = "aarch64",
        all(target_arch = "x86_64", target_feature = "sse2"),
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
    use crate::arch::Kernel;

    /// Dispatch reads [`arch::kernel`] rather than re-deriving the `cfg` ladder and
    /// the feature probe, so this classifier and [`crate::shuffle::refutes`] can
    /// never independently disagree about the target's silicon — which is the reason
    /// [`crate::arch`] exists.
    pub fn find(lo: &[u8; 16], hi: &[u8; 16], hay: &[u8]) -> Option<usize> {
        match arch::kernel() {
            #[cfg(target_arch = "aarch64")]
            // SAFETY: this arm is only reachable under `#[cfg(target_arch =
            // "aarch64")]`, where NEON is baseline — exactly `arch::neon::classify`'s
            // own precondition.
            Kernel::Neon => unsafe { arch::neon::classify(lo, hi, hay) },
            #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
            // SAFETY: `kernel()` names `Avx512` only where the probe confirmed both
            // leaf-7 bits, `OSXSAVE` and all five `XCR0` bits — exactly
            // `arch::avx512::classify`'s own precondition. `arch::force` can only name a
            // kernel that same probe admitted.
            Kernel::Avx512 => unsafe { arch::avx512::classify(lo, hi, hay) },
            #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
            // SAFETY: `kernel()` names `Avx2` only where the probe confirmed the
            // silicon, `OSXSAVE` and `XCR0` — exactly `arch::avx2::classify`'s own
            // precondition. `arch::force` can only name a kernel that same probe
            // admitted.
            Kernel::Avx2 => unsafe { arch::avx2::classify(lo, hi, hay) },
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            // SAFETY: this arm is only reachable under `target_feature = "simd128"`,
            // which is the whole of `arch::simd128::classify`'s precondition.
            Kernel::Simd128 => unsafe { arch::simd128::classify(lo, hi, hay) },
            #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
            // SAFETY: `kernel()` returns `Ssse3` only after its `CPUID` probe
            // confirmed the CPU has it — exactly `arch::ssse3::classify`'s own
            // precondition.
            Kernel::Ssse3 => unsafe { arch::ssse3::classify(lo, hi, hay) },
            _ => scalar(lo, hi, hay),
        }
    }

    /// The meaning of the two tables, spelled out once.
    pub fn member(lo: &[u8; 16], hi: &[u8; 16], b: u8) -> bool {
        lo[usize::from(b & 0xF)] & hi[usize::from(b >> 4)] != 0
    }

    pub fn scalar(lo: &[u8; 16], hi: &[u8; 16], hay: &[u8]) -> Option<usize> {
        hay.iter().position(|&b| member(lo, hi, b))
    }

    /// The `hay.len() % step` bytes no whole vector step covers. Called back into from
    /// every vector classifier in [`crate::arch`], so the tail is defined once against
    /// the same reference `scalar` the differential tests hold them to — and therefore
    /// present only where one of them is.
    ///
    /// `step` is passed in rather than read from [`arch::STEP`] because the classifiers
    /// no longer share one width: a 256- or 512-bit step leaves a different remainder
    /// than a 128-bit one, and a tail that assumed the narrowest would silently drop up
    /// to forty-eight bytes of the widest kernel's haystack.
    #[cfg(any(
        target_arch = "aarch64",
        all(target_arch = "x86_64", target_feature = "sse2"),
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
    pub(crate) fn tail(lo: &[u8; 16], hi: &[u8; 16], hay: &[u8], step: usize) -> Option<usize> {
        let done = hay.len() - hay.len() % step;
        scalar(lo, hi, &hay[done..]).map(|i| done + i)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    /// Every set shape the classifier claims, plus the two it must refuse.
    fn shapes() -> Vec<Vec<u8>> {
        vec![
            vec![],
            vec![b'W'],
            vec![b'a', b'b'],
            vec![b'a', b'b', b'g'],
            (b'0'..=b'9').collect(),
            (b'A'..=b'Z').collect(),
            (b'0'..=b'9')
                .chain(b'a'..=b'f')
                .chain(b'A'..=b'F')
                .collect(),
            vec![0x00, 0x01, 0x7F, b'\n'],
            (0..0x80u8).collect(),
        ]
    }

    fn skip_over(escape: &[u8]) -> Option<Skip> {
        // Row `b` sends block 0 away iff `b` escapes, which is the shape
        // `Skip::of` reads out of a real quotient.
        let mut rows = [[0u8; 16]; 256];
        for &b in escape {
            rows[usize::from(b)][0] = 1;
        }
        Skip::of(&rows, 0)
    }

    /// The claim the whole module rests on: the fast path and the definition agree
    /// on every alignment, so no skip can step over an escape byte. Offsets are
    /// swept because the vector path only sees whole 16-byte blocks and the bug
    /// this guards against lives in the tail.
    #[test]
    fn every_instrument_agrees_with_the_definition() {
        let mut hay = vec![0u8; 512];
        for (i, slot) in hay.iter_mut().enumerate() {
            // A deterministic non-uniform filler: mostly lowercase, so a set of
            // digits or uppercase is genuinely rare and the runs are long.
            *slot = match i % 17 {
                0 => b'A' + (i % 26) as u8,
                1 => b'0' + (i % 10) as u8,
                7 => 0x80 | (i % 128) as u8,
                _ => b'a' + (i % 26) as u8,
            };
        }
        for escape in shapes() {
            let Some(skip) = skip_over(&escape) else {
                continue;
            };
            for start in 0..64 {
                for len in [0usize, 1, 15, 16, 17, 31, 33, 64, 129, 255] {
                    let end = (start + len).min(hay.len());
                    let slice = &hay[start.min(hay.len())..end];
                    assert_eq!(
                        skip.find(slice),
                        skip.find_scalar(slice),
                        "escape={escape:?} start={start} len={len}"
                    );
                }
            }
        }
    }

    /// The classifier must be exact on the alphabet, not merely close: one byte
    /// wrongly called a non-member is one escape the sieve walks past.
    #[test]
    fn the_classifier_admits_exactly_the_set() {
        for escape in shapes() {
            let Some(skip) = skip_over(&escape) else {
                continue;
            };
            for b in 0..=255u8 {
                let found = skip.find(&[b]) == Some(0);
                assert_eq!(
                    found,
                    escape.contains(&b),
                    "byte {b:#04x} misclassified for escape={escape:?}"
                );
            }
        }
    }

    /// A set the tables cannot represent exactly has to be refused outright. The
    /// hazard is a classifier that silently reports "not a member" for a high byte
    /// and hands back a skip that runs past a real escape.
    #[test]
    fn a_non_ascii_escape_set_is_declined_rather_than_approximated() {
        assert!(skip_over(&[b'a', b'b', b'c', 0xC3]).is_none());
        assert!(skip_over(&(0..=255u8).collect::<Vec<_>>()).is_none());
    }

    /// An empty escape set is a sink, and a sink is the strongest answer this
    /// module can give: no byte anywhere will move the run.
    #[test]
    fn a_block_nothing_leaves_never_finds_an_escape() {
        let skip = skip_over(&[]).expect("an absorbing block still yields a skip");
        assert_eq!(skip.find(&[0u8; 300]), None);
        assert_eq!(skip.find_scalar(&[0u8; 300]), None);
    }

    /// A deterministic xorshift, so a failure names a seed a reader can replay
    /// rather than a mood the machine was in.
    fn rolls(seed: u64) -> impl FnMut() -> u64 {
        let mut s = seed | 1;
        move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        }
    }

    /// The sets above are nine the author thought of. The classifier's exactness is a
    /// claim about *all* of them, and nibble aliasing is precisely the bug an author's
    /// intuition picks sets to avoid — so sweep the space instead of sampling taste.
    ///
    /// Densities are swept deliberately: a one-byte set exercises `memchr`, a
    /// thirty-byte set exercises the nibble tables, and the boundary between them is
    /// where an instrument gets chosen wrongly.
    #[test]
    fn the_classifier_is_exact_on_sets_nobody_hand_picked() {
        let mut next = rolls(0x5E1F_C0DE_1234_5678);
        for trial in 0..2048 {
            let width = 1 + (trial % 96);
            let mut escape = Vec::with_capacity(width);
            while escape.len() < width {
                let b = (next() % 128) as u8; // ASCII: the domain the tables represent
                if !escape.contains(&b) {
                    escape.push(b);
                }
            }
            let Some(skip) = skip_over(&escape) else {
                continue; // refused, which is always a sound answer
            };
            for b in 0..=255u8 {
                assert_eq!(
                    skip.find(&[b]) == Some(0),
                    escape.contains(&b),
                    "trial {trial}: byte {b:#04x} misclassified for {escape:?}"
                );
            }
        }
    }

    /// The tail is where a vector search goes wrong, and it goes wrong silently: a
    /// dropped remainder does not crash, it reports `None` and the sieve walks straight
    /// past a real escape into a wrong refutation.
    ///
    /// So plant one escape byte at *every* offset of every length across the chunk
    /// boundaries and demand the exact index back. A search that handled only whole
    /// 16-byte blocks would survive both tests above and die on the first row here.
    #[test]
    fn an_escape_is_found_at_every_offset_of_every_length() {
        for escape in shapes() {
            let Some(skip) = skip_over(&escape) else {
                continue;
            };
            let Some(&needle) = escape.first() else {
                continue; // the sink has nothing to plant
            };
            // A filler that is guaranteed *not* to escape, so the planted byte is the
            // only right answer and an off-by-one cannot coincidentally agree.
            let Some(filler) = (0..=255u8).find(|b| !escape.contains(b)) else {
                continue;
            };
            for len in 1..=72usize {
                for at in 0..len {
                    let mut hay = vec![filler; len];
                    hay[at] = needle;
                    assert_eq!(
                        skip.find(&hay),
                        Some(at),
                        "escape={escape:?} len={len} planted at {at}"
                    );
                    assert_eq!(skip.find(&hay), skip.find_scalar(&hay));
                }
            }
        }
    }
}
