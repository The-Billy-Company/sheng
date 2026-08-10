//! The worth-test arithmetic itself: survival, the cost inequality, and the
//! recorded evidence a caller can explain a decision from.

/// The haystack length the survival term is amortized over when the caller names
/// none. Arming is judged against a **whole document** rather than a line for two
/// reasons that happen to agree: the sieve has one kernel and serves every caller
/// from it, and the estimate feeding `f` is known to be optimistic even under the
/// persistence prior, so requiring the bound to clear at 4 KiB is how a structural
/// estimate buys margin against its own residual bias without observing traffic.
///
/// Every cost this length multiplies is per-byte, with no constant term, and that was
/// measured rather than assumed. Both loops do pay something per *call* — a dozen
/// nanoseconds or so, for table loads and a masked tail — but they pay it in nearly
/// equal measure, so it cancels in an inequality drawn between them, and the residual
/// is under a percent of the sieve side here: an order of magnitude inside [`MARGIN`].
/// It is proportionally much larger against an accelerated rival, whose per-byte cost
/// is the smaller number it hides in — but charging it there would only ever argue
/// *for* arming, against a rival the sieve already loses to by more than an order of
/// magnitude, so leaving it out is also the conservative direction.
///
/// What does not survive shortening is the advantage itself. This length sits where
/// that edge has saturated — it holds within a couple of percent from here up to 64 KiB
/// and down to a kilobyte, then falls away, to under half by 64 bytes. So a caller who
/// really searches 64-byte records cannot read a verdict taken at 4 KiB as one taken at
/// theirs, which is what [`VALIDITY_FLOOR`] refuses, and `examples/bench.rs` prints the
/// sweep both constants are read off.
pub const NOMINAL_LEN: f64 = 4096.0;

/// The shortest document a verdict may be taken at, below which the gate declines
/// rather than extrapolate.
///
/// Two things the model leaves out grow as the document shrinks, they push the same
/// way, and neither is a coefficient that could simply be re-minted:
///
/// * The sieve pays a **per-call** cost — table loads, a masked tail — worth a few
///   nanoseconds. Under a percent of its per-byte price at [`NOMINAL_LEN`] and around
///   half of it at 64 bytes.
/// * The rival gets **cheaper per byte** as records shorten, which is the larger effect
///   and the counter-intuitive one. Consecutive searches over short records are
///   independent dependency chains, so a wide core overlaps them in a way it cannot
///   overlap one long walk: the same engine measures 1.27 ns/B over 4 KiB records and
///   0.71 over 64-byte ones.
///
/// The first inflates the sieve's cost and the second deflates the rival's, so both
/// inflate the *predicted* speedup, and a caller near the threshold would be armed on
/// the difference. The second is also why the sieve's own length curve — which
/// `examples/bench.rs` used to print alone — cannot settle this: the gate compares a
/// ratio, and the leg that moves most is the other one.
///
/// [`MARGIN`] is the yardstick for where that becomes intolerable rather than merely
/// imprecise, which is what puts this constant here instead of at a rounder number, and
/// the crossing is measured rather than argued. Swept over one machine's records
/// (`cargo run --release --example bench`), the sieve's edge over a walking rival sits
/// within a couple of percent of its nominal value from 65,536 bytes down to 1,024, is
/// **16% under** it here at 256, and **39% under** at 128 — so the margin already being
/// held back absorbs every length at or above this one and none below it. Under the
/// floor, arming is decided by the terms the model does not carry, and the honest answer
/// is that this row was not measured over documents like these.
///
/// Declining costs such a caller a real speedup, sometimes several-fold; it is still the
/// right way to be wrong, for the reason [`MARGIN`] gives — a decline costs the speedup
/// once, an arm on noise costs a full sieve pass per document forever.
///
/// # Why the terms are not simply minted
///
/// Because measuring them retired them. The per-call cost is a real constant and could
/// be carried, but adding it alone would make the model more conservative without making
/// it more accurate, since the rival's overlap is both larger and in the opposite
/// direction. And that one is not a coefficient at all — it is a reorder window
/// saturating, so a number fitted to one machine's window describes no other machine's.
/// A floor states the same fact without pretending to a portability it does not have.
pub const VALIDITY_FLOOR: f64 = 256.0;

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

/// How many rivals a pre-pass fronts, as the arithmetic reads it. Clamped to at
/// least one: a sieve in front of nothing has no rival to be cheaper than, and
/// zero here would make the gate's right side zero and decline everything — a
/// correct answer arrived at for the wrong reason.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn fanout(rivals: usize) -> f64 {
    rivals.max(1) as f64
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
    /// How many searches one refutation skips. See [`crate::Policy::rivals`].
    ///
    /// One for a caller with one pattern, which is the shape every term above was
    /// written for. Above one it is the *only* term that divides the sieve's price
    /// rather than multiplying it, and that asymmetry is the whole reason it exists —
    /// a pre-pass paid once against verification paid `rivals` times.
    pub rivals: usize,
    /// What the caller's cheapest **exact** alternative costs per byte, in total —
    /// the pipeline they would run if this sieve did not exist. See
    /// [`crate::Bypass`].
    ///
    /// Infinite when there is none, which is the arithmetic that reproduces the gate
    /// as it read before this term: a comparison against doing nothing.
    pub bypass: f64,
}

impl CostFact {
    /// The gate's left side: the pre-pass, plus whatever survives it paying the same
    /// alternative the right side pays in full.
    #[must_use]
    pub fn total(self) -> f64 {
        self.sieve + survival(self.fallthrough, self.len) * self.unfiltered()
    }

    /// The gate's right side: the cheapest thing the caller could do **without** this
    /// sieve — every rival walking every byte, or the exact pre-pass that would spare
    /// them, whichever is less.
    ///
    /// # Why this is a minimum and not a product
    ///
    /// A sieve is only ever worth the work it retires, and the work it retires is the
    /// work the caller would *otherwise have done* — not the most expensive thing they
    /// could name. Those differ by orders of magnitude in the one case a caller most
    /// wants to reach for [`crate::Rival::Walks`]: a survivor that costs an OCR pass or
    /// a network round trip is not, in any sane pipeline, paid on every document,
    /// because a regex engine can decide the question exactly for a hundredth of the
    /// price. Fronting the confirm directly is then a comparison against a pipeline
    /// nobody runs, and it arms sieves that lose to the engine by two orders of
    /// magnitude.
    ///
    /// Taking the minimum is what refuses that comparison. It needs no term for the
    /// true hit rate, because everything downstream of an exact decision is paid at
    /// that rate on **both** sides of the inequality and cancels: a sieve in front of
    /// an exact pre-pass changes the price of the pre-pass and nothing after it. What
    /// is left is the only thing a refutation can move.
    ///
    /// A quantity that is not a cost poisons the answer rather than hiding behind the
    /// other one. [`f64::min`] prefers its non-NaN operand, which would let a nonsense
    /// rival price be silently replaced by a real bypass and arm on it.
    #[must_use]
    pub fn unfiltered(self) -> f64 {
        let blind = fanout(self.rivals) * self.rival;
        if blind.is_nan() || self.bypass.is_nan() {
            return f64::NAN;
        }
        blind.min(self.bypass)
    }

    /// The most this sieve could **ever** be worth, at any rival price and any slate
    /// size: the reciprocal of the share of documents it fails to retire.
    ///
    /// Both amortizing terms — [`crate::Policy::rival`] and [`crate::Policy::rivals`] —
    /// work by shrinking the pre-pass against the work it fronts, so the speedup they
    /// drive is `unfiltered / (sieve + survival * unfiltered)` and every one of them
    /// converges on `1 / survival` from below. Nothing a caller can buy goes past it.
    ///
    /// It is the number to read before tuning anything. A pattern declining at 1.1x
    /// with a ceiling of 1.15x is finished — no confirm is expensive enough and no
    /// slate is long enough. The identical decline with a ceiling of 11x is a pattern
    /// whose survival is being decided by [`crate::Policy::len`], and shortening the
    /// records is the lever that was actually available.
    #[must_use]
    pub fn ceiling(self) -> f64 {
        1.0 / survival(self.fallthrough, self.len)
    }

    /// Whether fronting the rivals with this sieve is cheaper than not, by more than
    /// the coefficients behind the comparison can resolve. See [`MARGIN`].
    ///
    /// # Why the comparison is guarded rather than taken bare
    ///
    /// An inequality between two costs decides anything only while both sides really are
    /// costs, and not every term reaching here comes from a mint. [`crate::Policy::rival`]
    /// and [`crate::Policy::bypass`] take prices the caller states outright, and a
    /// [`super::Calibration`] can be hand-built for a machine this crate never measured —
    /// so a negative, infinite, or NaN quantity is reachable without anything having gone
    /// wrong internally.
    ///
    /// Bare, the comparison arms on the first of those. A negative rival makes
    /// `unfiltered` negative, and a filter leaky enough that survival is ~1 has
    /// `total ≈ sieve + rival`, so `(sieve + r)(1 + MARGIN) < r` holds for any `sieve`
    /// under `-MARGIN * r` — a sieve armed by a price that is not a price, which is the
    /// one outcome this whole module exists to prevent. Requiring both sides to be real
    /// costs before comparing them costs two predicates and moves every such input to the
    /// decline it should always have been. NaN is covered by the same guard for free,
    /// every ordering on it already being false.
    #[must_use]
    pub fn pays(self) -> bool {
        let (total, unfiltered) = (self.total(), self.unfiltered());
        total >= 0.0 && unfiltered.is_finite() && total * (1.0 + MARGIN) < unfiltered
    }

    /// How much cheaper, as the speedup a caller would feel. Below 1.0 the sieve
    /// is overhead.
    #[must_use]
    pub fn speedup(self) -> f64 {
        self.unfiltered() / self.total()
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
            rivals: 1,
            bypass: f64::INFINITY,
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

    /// The fan-out term has one job — amortize a pre-pass paid once over verification
    /// paid many times — and three ways to get it wrong, each pinned here.
    ///
    /// It must be a no-op at one rival, or every verdict this crate has ever measured
    /// moved when the field was added. It must be **monotone**, since a caller adding a
    /// pattern to a slate cannot make a refutation worth less. And it must not become a
    /// way to arm a filter that refutes nothing: fan-out divides the sieve's price, not
    /// the survival term, so a filter whose survivors still drag every document into
    /// every verification is exactly as unprofitable across a slate as it is alone.
    #[test]
    fn fanning_out_amortizes_the_pre_pass_and_nothing_else() {
        let at = |rivals: usize, fallthrough: f64| CostFact {
            fallthrough,
            len: NOMINAL_LEN,
            // A sieve that costs four times what one rival does: hopeless alone,
            // and the shape a slate is supposed to rescue.
            sieve: 4.0,
            rival: 1.0,
            rivals,
            bypass: f64::INFINITY,
        };

        // One rival is the arithmetic that shipped before this term existed.
        let one = at(1, 0.0);
        assert_eq!(one.total(), 4.0);
        assert_eq!(one.unfiltered(), 1.0);
        assert!(!one.pays(), "a sieve four times its rival cannot pay alone");

        // A slate long enough to amortize it, and the crossing is where the arithmetic
        // says it is rather than wherever it lands: sieve/rivals must clear the margin.
        assert!(!at(4, 0.0).pays(), "at parity the margin still binds");
        assert!(
            at(8, 0.0).pays(),
            "eight rivals amortize a fourfold pre-pass"
        );

        // Monotone in the slate size, over both a selective and a leaky filter.
        for fallthrough in [0.0, 1e-9, 1e-4] {
            let mut previous = f64::MIN;
            for rivals in 1..=64 {
                let speedup = at(rivals, fallthrough).speedup();
                assert!(
                    speedup >= previous,
                    "speedup fell from {previous} to {speedup} at {rivals} rivals"
                );
                previous = speedup;
            }
        }

        // A filter that retires nothing is beyond rescue at any slate size, because
        // fan-out multiplies both sides of the survival term.
        for rivals in [1usize, 2, 16, 1024] {
            assert!(
                !at(rivals, 1.0).pays(),
                "a filter that passes every position armed at {rivals} rivals"
            );
        }

        // Zero rivals is a caller error, and it must read as "one" rather than as a
        // right-hand side of zero — which would decline for the wrong reason and make
        // `speedup` a division by zero.
        assert_eq!(at(0, 0.0).total(), at(1, 0.0).total());
        assert_eq!(at(0, 0.0).unfiltered(), at(1, 0.0).unfiltered());
    }

    #[test]
    fn a_sieve_never_pays_in_front_of_a_free_rival() {
        let fact = CostFact {
            fallthrough: 0.0,
            len: NOMINAL_LEN,
            sieve: 0.5,
            rival: 0.1,
            rivals: 1,
            bypass: f64::INFINITY,
        };
        assert!(
            !fact.pays(),
            "a cheap rival must stand the sieve down on its own"
        );
    }

    /// The hole the baseline closes, in the arithmetic that revealed it.
    ///
    /// A confirm at five hundred walks a byte, fronted by a filter half of whose
    /// documents survive, doubles the throughput of a pipeline that puts every document
    /// through the confirm. Nobody runs that pipeline. They run the engine first, which
    /// decides the same question exactly, and then the same sieve is merely a second
    /// filter competing with the engine at the engine's own job — where it draws against
    /// a walk and loses by two orders of magnitude to a `memchr`.
    ///
    /// The confirm's price never appears in the honest verdict, which is the part worth
    /// seeing computed: once an exact pre-pass is available it is what the survival term
    /// multiplies, and five hundred walks becomes a number in a `min` that never wins.
    #[test]
    fn an_expensive_confirm_is_measured_against_the_engine_that_would_have_screened_it() {
        // Half of all documents survive at 4 KiB.
        let strawman = CostFact {
            fallthrough: 1.692e-4,
            len: NOMINAL_LEN,
            sieve: 0.5,
            rival: 512.0,
            rivals: 1,
            bypass: f64::INFINITY,
        };
        assert!(
            (survival(strawman.fallthrough, strawman.len) - 0.5).abs() < 0.01,
            "the premise moved: this fixture is written around a coin-flip survival"
        );
        assert!(
            strawman.pays() && strawman.speedup() > 1.9,
            "the comparison against doing nothing was never the doubtful one: {:.2}x",
            strawman.speedup()
        );

        // The same sieve, the same confirm, against an engine committed to a walk.
        let walking = CostFact {
            bypass: 1.0,
            ..strawman
        };
        assert_eq!(
            walking.unfiltered(),
            1.0,
            "the baseline must be the cheaper of the two"
        );
        assert!(
            !walking.pays(),
            "a sieve that only draws with the engine armed anyway: {:.3}x",
            walking.speedup()
        );

        // And against one that can `memchr`, which is where the two orders of magnitude
        // in this objection actually live.
        let accelerated = CostFact {
            bypass: 0.0175,
            ..strawman
        };
        assert!(
            accelerated.speedup() < 0.05,
            "a sieve fronting a skipping engine should lose by ~100x, not {:.3}x",
            accelerated.speedup()
        );

        // And the term is inert where it should be: a bypass no cheaper than the blind
        // pipeline changes no verdict at all, which is what makes `Bypass::Engines` safe
        // as a default under `Rival::Engine`.
        for bypass in [512.0f64, 1024.0, f64::INFINITY] {
            let inert = CostFact { bypass, ..strawman };
            assert_eq!(
                (inert.pays(), inert.speedup()),
                (strawman.pays(), strawman.speedup()),
                "a baseline at {bypass} moved a verdict it cannot improve on"
            );
        }
    }

    /// A price that is not a price must not be able to hide behind one that is.
    ///
    /// [`f64::min`] returns its non-NaN operand, so the obvious spelling of the
    /// baseline would quietly discard a nonsense rival and decide on the bypass — and
    /// a nonsense bypass would be discarded in favor of the rival. Either way the guard
    /// in [`CostFact::pays`] never sees the defect it exists to catch.
    #[test]
    fn a_nonsense_price_poisons_the_baseline_rather_than_being_dropped_from_it() {
        let sound = CostFact {
            fallthrough: 0.0,
            len: NOMINAL_LEN,
            sieve: 0.1,
            rival: 4.0,
            rivals: 1,
            bypass: 4.0,
        };
        assert!(sound.pays(), "the premise: this one arms");
        for poisoned in [
            CostFact {
                rival: f64::NAN,
                ..sound
            },
            CostFact {
                bypass: f64::NAN,
                ..sound
            },
        ] {
            assert!(poisoned.unfiltered().is_nan());
            assert!(
                !poisoned.pays(),
                "a NaN price was dropped in favor of the other side and armed"
            );
        }
    }

    /// The ceiling has to be exactly what every amortizing term converges on, not
    /// merely near it — otherwise it is a slogan rather than a bound a caller can stop
    /// tuning against.
    #[test]
    fn no_slate_size_and_no_rival_price_passes_the_ceiling() {
        for fallthrough in [1e-6, 1e-4, 1e-3] {
            let at = |rivals: usize| CostFact {
                fallthrough,
                len: NOMINAL_LEN,
                sieve: 4.0,
                rival: 1.0,
                rivals,
                bypass: f64::INFINITY,
            };
            let ceiling = at(1).ceiling();
            for rivals in [1usize, 2, 64, 4096, 1 << 20] {
                assert!(
                    at(rivals).speedup() < ceiling,
                    "f={fallthrough} at {rivals} rivals reached {} past a ceiling of \
                     {ceiling}",
                    at(rivals).speedup()
                );
            }
            // Approached, not merely respected: a bound nothing gets near is not the
            // bound, it is an over-estimate that would advise a caller to keep tuning.
            let far = at(1 << 30).speedup();
            assert!(
                far > ceiling * 0.999,
                "a slate of a billion reached only {far} of a {ceiling} ceiling"
            );
        }
        // A filter nothing survives has no ceiling, and must say so rather than
        // dividing by zero into a number.
        let perfect = CostFact {
            fallthrough: 0.0,
            len: NOMINAL_LEN,
            sieve: 1.0,
            rival: 1.0,
            rivals: 1,
            bypass: f64::INFINITY,
        };
        assert!(perfect.ceiling().is_infinite());
    }
}
