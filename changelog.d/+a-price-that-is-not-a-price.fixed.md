`CostFact::pays` compared two costs without first checking they were costs, and a negative
one armed a sieve. With a filter leaky enough that survival is ~1 the left side is
`sieve + rival`, so `(sieve + r)(1 + MARGIN) < r` holds for any `sieve` under `-MARGIN * r`
— the inequality inverts and admits a filter that retires nothing, which is the single
outcome the whole `price` module exists to prevent.

Nothing internal could produce such a number, which is why it survived: every coefficient
reaching the gate came from a mint, and `Calibration::is_measured` already refuses a row
whose walk or skip price is not positive. It became reachable the moment a caller could
state a rival's price outright, and it was reachable before that through a hand-built
`Calibration` — the seam `tests/policy.rs` exists to keep open.

So the guard is at the comparison rather than at either entry point, where it covers every
source of the defect instead of the two that are spelled out: `pays` now requires the total
to be a non-negative real and the unfiltered side to be finite before comparing them. NaN
comes along for free, every ordering on it already being false, as does the `0 * infinity`
the survival term produces for a perfectly selective filter against an infinite rival.
Verdicts on real coefficients are unchanged — the guard only ever rejects inputs that were
never prices.
