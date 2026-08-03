//! The worth-test arithmetic itself: survival, the cost inequality, and the
//! recorded evidence a caller can explain a decision from.

/// The haystack length the survival term is amortized over when the caller names
/// none. Arming is judged against a **whole document** rather than a line for two
/// reasons that happen to agree: the sieve has one kernel and serves every caller
/// from it, and the estimate feeding `f` is known to be optimistic even under the
/// persistence prior, so requiring the bound to clear at 4 KiB is how a structural
/// estimate buys margin against its own residual bias without observing traffic.
pub const NOMINAL_LEN: f64 = 4096.0;

/// The share of haystacks of `len` bytes that survive a filter passing `f` of
/// positions. One survivor costs the whole haystack, which is why this rises so
/// much faster than `f` does.
#[must_use]
pub fn survival(f: f64, len: f64) -> f64 {
    1.0 - (1.0 - f).powf(len)
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

    /// Whether fronting the rival with this sieve is cheaper than not.
    #[must_use]
    pub fn pays(self) -> bool {
        self.total() < self.rival
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
