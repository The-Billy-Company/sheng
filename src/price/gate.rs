//! The worth-test arithmetic itself: survival, the cost inequality, and the
//! recorded evidence a caller can explain a decision from.

/// The haystack length the survival term is amortized over when the caller names
/// none. Arming is judged against a **whole document** rather than a line for two
/// reasons that happen to agree: the sieve has one kernel and serves every caller
/// from it, and the estimate feeding `f` is known to be optimistic even under the
/// persistence prior, so requiring the bound to clear at 4 KiB is how a structural
/// estimate buys margin against its own residual bias without observing traffic.
pub const NOMINAL_LEN: f64 = 4096.0;

/// How much a modeled speedup must beat 1.0 by before it counts as a decision
/// rather than a coincidence.
///
/// Every term the gate compares is a measurement, and a minted row states its own
/// run-to-run spread out loud — double-digit percent on the per-byte figures, more
/// on `dfa_excursion`. A verdict is a *ratio* of two of them and inherits both
/// spreads, so arming at `speedup > 1.0` bets on differences the mint cannot
/// resolve. Scale invariance does not help here: it cancels a factor common to every
/// coefficient, and this is the part that is *not* common.
///
/// The bet is also asymmetric, which is what settles the direction. A sieve that
/// declines costs the caller the speedup it would have had and nothing else; a sieve
/// that arms on noise costs the whole sieve pass on every document it then fails to
/// refute, forever. So the margin is required of the sieve, not split between them.
///
/// Two patterns are this hazard in its pure form. `WalletService` and `foo[^\n]*bar`
/// both elect a `memchr` skip over the *same byte the engine already accelerates
/// on* — so the streaming halves of the two prices are the same loop over the same
/// needle and cancel exactly. The entire modeled edge is
/// [`super::Calibration::skip_excursion`] sitting a hair under `dfa_excursion`.
/// Both scored a near-parity win, both armed, and both then measured as losses or
/// wins depending on residency — a coin flip, which is exactly what a near-parity
/// prediction drawn from noisy inputs should look like.
pub const MARGIN: f64 = 0.25;

/// The share of haystacks of `len` bytes that survive a filter passing `f` of
/// positions. One survivor costs the whole haystack, which is why this rises so
/// much faster than `f` does.
#[must_use]
pub fn survival(f: f64, len: f64) -> f64 {
    1.0 - pow(1.0 - f, len)
}

/// `x` raised to a whole number of byte positions, by squaring.
///
/// This is the crate's only exponentiation, and doing it here rather than through
/// `f64::powf` is what keeps the arithmetic `core`-only: every float operation in
/// `sheng` is now `+ - * /` and a comparison, so a sieve can be priced on a target
/// with no `std` and no math library — which is the least a kernel this size should
/// be able to promise.
///
/// The exponent is a **count of bytes**, so it is taken as the whole positions it
/// names rather than interpolated between them; `len` stays an `f64` because that is
/// what every caller already has and what `NOMINAL_LEN` already is. A nonsense
/// length cannot become a nonsense loop: the cast saturates, so negatives and NaN
/// read as zero bytes and the widest conceivable length is 64 squarings.
///
/// Squaring accumulates roughly `log2(len)` roundings where a correctly-rounded
/// `powf` accumulates one — about 1e-15 relative at the shipped 4096, which is
/// twelve orders of magnitude under the modeling error in the `f` being raised.
fn pow(mut x: f64, len: f64) -> f64 {
    let mut n = len as u64;
    let mut acc = 1.0;
    while n > 0 {
        if n & 1 == 1 {
            acc *= x;
        }
        x *= x;
        n >>= 1;
    }
    acc
}

/// The exact arithmetic the gate applies, retained whether the candidate arms or
/// declines so a caller can explain the decision without reconstructing it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostFact {
    /// Per-position fallthrough under the pessimistic prior.
    pub fallthrough: f64,
    /// Haystack length the survival term is amortized over.
    pub len: f64,
    /// The sieve's own per-byte price at its conjunct count.
    pub sieve: f64,
    /// The confirming engine's per-byte price, read from the engine.
    pub rival: f64,
}

impl CostFact {
    /// The gate's left side: the pre-pass, plus verifying what survives it.
    #[must_use]
    pub fn total(self) -> f64 {
        self.sieve + survival(self.fallthrough, self.len) * self.rival
    }

    /// Whether fronting the rival with this sieve is cheaper than not, by more than
    /// the coefficients behind the comparison can resolve. See [`MARGIN`].
    #[must_use]
    pub fn pays(self) -> bool {
        self.total() * (1.0 + MARGIN) < self.rival
    }

    /// How much cheaper, as the speedup a caller would feel. Below 1.0 the sieve
    /// is overhead.
    #[must_use]
    pub fn speedup(self) -> f64 {
        self.rival / self.total()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survival_rises_faster_than_fallthrough() {
        // The lesson that killed position-rejection as a gate: rejecting 99% of
        // positions still keeps most documents.
        assert!(survival(0.01, NOMINAL_LEN) > 0.99);
        assert!(survival(1e-6, NOMINAL_LEN) < 0.01);
    }

    /// The replacement for `f64::powf` has to *be* `f64::powf` at every length and
    /// rate the gate actually reads, not merely close in the neighborhood somebody
    /// checked. So hold it to the standard library over the whole range — including
    /// the tiny rates where `survival` decides everything and where a naive
    /// exponentiation would be most tempted to collapse to zero or one.
    ///
    /// Only runs where there is a `std` to disagree with, which is the point: this
    /// pins the `core` arithmetic to the implementation it replaced.
    #[cfg(feature = "std")]
    #[test]
    fn squaring_agrees_with_the_standard_library_it_replaces() {
        for len in [
            0.0f64,
            1.0,
            2.0,
            3.0,
            15.0,
            64.0,
            1024.0,
            NOMINAL_LEN,
            65536.0,
        ] {
            for f in [0.0f64, 1e-30, 1e-12, 1e-6, 1e-3, 0.01, 0.5, 0.99, 1.0] {
                let (base, mine, theirs) = (1.0 - f, pow(1.0 - f, len), (1.0 - f).powf(len));
                let scale = theirs.abs().max(f64::MIN_POSITIVE);
                assert!(
                    (mine - theirs).abs() / scale < 1e-12,
                    "{base}^{len}: squaring {mine:e} against powf {theirs:e}"
                );
            }
        }
    }

    /// A length that is not a count of bytes must not become a loop that never ends
    /// or an answer that is not a probability. The saturating cast is the guard, and
    /// a guard nobody exercises is a guess.
    #[test]
    fn a_nonsense_length_yields_no_survival_rather_than_a_hang() {
        for len in [-1.0f64, -0.0, f64::NAN, f64::NEG_INFINITY] {
            assert_eq!(survival(0.5, len), 0.0, "len={len} counted some bytes");
        }
        // The widest length representable is 64 squarings, not an unbounded walk.
        assert_eq!(survival(0.5, f64::INFINITY), 1.0);
    }

    /// The margin has to bind on the side that can lose. A sieve modeled at a hair
    /// under parity must decline, one modeled at a hair over must *also* decline
    /// because the hair is smaller than the coefficients that produced it, and one
    /// clear of the margin must still arm — otherwise the fix is a disarm.
    #[test]
    fn an_edge_inside_the_coefficient_noise_does_not_arm() {
        let at = |speedup: f64| CostFact {
            fallthrough: 0.0,
            len: NOMINAL_LEN,
            sieve: 1.0,
            rival: speedup,
        };
        assert!(!at(0.99).pays(), "a modeled loss must never arm");
        assert!(
            !at(1.01).pays(),
            "a hair over parity is the shape `WalletService` armed on: an edge the mint cannot see"
        );
        assert!(
            !at(1.0 + MARGIN).pays(),
            "the margin is a floor, not a target"
        );
        assert!(
            at(1.0 + MARGIN + 1e-9).pays(),
            "a sieve clear of the noise must still arm, or this is a disarm"
        );
        // Scale invariance survives the margin: it is a ratio test either way.
        for k in [0.25f64, 1.0, 91.0] {
            let scaled = CostFact {
                sieve: 1.0 * k,
                rival: 2.0 * k,
                ..at(2.0)
            };
            assert!(
                scaled.pays(),
                "k={k} moved a decision the margin should not"
            );
        }
    }

    #[test]
    fn a_sieve_never_pays_in_front_of_a_free_rival() {
        let fact = CostFact {
            fallthrough: 0.0,
            len: NOMINAL_LEN,
            sieve: 0.5,
            rival: 0.1,
        };
        assert!(
            !fact.pays(),
            "a cheap rival must stand the sieve down on its own"
        );
    }
}
