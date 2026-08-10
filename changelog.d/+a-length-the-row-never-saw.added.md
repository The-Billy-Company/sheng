A caller who declares documents shorter than the calibration was measured over now gets
`BuildError::Unmodeled` instead of a speedup figure, governed by the new
`price::VALIDITY_FLOOR`.

The gate charges every loop in ns/byte, so a document's price is linear in its length and
passes through the origin. Two things it therefore cannot see grow as the document
shrinks. Both loops pay a per-*call* cost — table loads, a masked tail, a restarted
`memchr` — worth under a percent of the sieve at `NOMINAL_LEN` but around half of it at 64
bytes. And the sieve's edge over a walking rival, flat from a few kilobytes up, collapses
to roughly a third of that by 64 bytes. Both inflate the predicted speedup, so both argue
for arming, and near the threshold a caller would have been armed on the difference.

`MARGIN` decides where that stops being imprecision and becomes the whole verdict, and the
crossing is measured — see *A slate measured rather than asserted*, which swept it: the
edge holds within a couple of percent down to a kilobyte, is 16% under nominal at 256
bytes, and 39% under at 128. So the existing cushion absorbs every length at or above the
floor and none below it, the floor sits there rather than at a rounder number, and under it
the answer is not "your
pattern does not pay" but "this row was not measured over documents like yours" — which is
why it is its own variant and not a `NotWorthIt`. `Gate::Ungated` consults no price and
ignores the floor, so a caller who knows their own traffic can still have the filter; what
is no longer on offer is a promise with no measurement behind it.

Measured rather than assumed, and the same sweep retired a coefficient before it was built:
a per-document fixed cost was the leading candidate for the missing term, and adding it
would not have moved the floor. It is real on the sieve's side and a few nanoseconds
across, but the larger short-record effect belongs to the rival and runs the other way, so
there is no constant to mint that makes the ratio right. `NOMINAL_LEN` records that
negative result, and why 4096 is the knee of the measured ratio rather than only a margin
against an optimistic fallthrough estimate.
