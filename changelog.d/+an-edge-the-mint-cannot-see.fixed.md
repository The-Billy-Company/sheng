The arming gate now requires a modeled speedup to clear 1.0 by `price::MARGIN` (25%)
rather than by any amount at all. Two patterns on the survey slate were arming on an
edge smaller than the measurement error of the coefficients that produced it, and then
losing.

`WalletService` scored 1.010x and `foo[^\n]*bar` scored 1.009x. Both elect a `memchr`
skip over **the same byte the engine already accelerates on** — escape set `['W']`
against start-state accelerator `"W"`, `['f']` against `"f"` — so the streaming halves
of the two prices are the same loop over the same needle and cancel exactly. What was
left carrying the verdict was `skip_excursion` sitting 3.5% under `dfa_excursion`, and
`price::MACOS_AARCH64_NEON` publishes those coefficients' own run-to-run spread as
~10% and ~21%. The gate was betting on a difference the mint cannot resolve.

It lost the bet about half the time, which is what a 1% prediction from ±20% inputs
should look like: measured end to end, the two came out **0.780x and 0.940x** against a
cache-resident corpus and **1.119x and 1.057x** against a memory-resident one. Neither
figure was ever outside its own measurement interval — `WalletService` read
0.978-1.250x on 22 MiB, which is a survey declining to say who won.

Scale invariance is what makes this a real gap rather than a conservative nudge. The
gate is provably indifferent to a factor common to every coefficient — clock, thermal
state, ambient load — so the residual it *is* exposed to is exactly the part that is
not common: the spread between two independently timed kernels. A margin is the only
place that can be accounted for, and the asymmetry settles its direction. A sieve that
declines costs the caller the speedup it would have had and nothing more; a sieve that
arms on noise costs a full sieve pass on every document it then fails to refute, for as
long as it is deployed. So the margin is required of the sieve rather than split.

What it costs and what it buys, on the same slate and machine:

- 0.3 MiB, cache-resident: geomean over armed rows **1.565x → 2.183x**, and two rows
  that measured below 1.0 are gone.
- 22.3 MiB, memory-resident: geomean **2.277x → 3.346x**, and the survey now reaches
  "every armed row cleared 1.000x" instead of reporting one row as undecided.
- Given up: 1.119x and 1.057x on 22 MiB, both inside their own intervals.

`skip.rs` has always stated the rule in prose — "a rival already `memchr`-ing the
identical byte cannot be beaten by a filter that has to find the same byte first" — and
`tests/policy.rs` now enforces it, checking both that the two sets coincide and that
the pattern declines. Deliberately as a test and not as a code path: a hard "same set,
therefore decline" would be **wrong**, because the two loops search the same needle but
excurse into different machines — the engine into a dense DFA whose table misses cache,
the sieve into sixteen blocks already in registers — so a genuinely cheaper excursion
is a genuinely cheaper sieve, and vetoing it would veto a real win. Coinciding escape
sets do not make a sieve worthless; they cancel the streaming term and leave the
verdict resting on one ratio of two noisy coefficients. That is a reason for the margin
to exist, not a second gate.

`BuildError::NotWorthIt` now names the bar it missed, so a decline reads
`1.010x, under the 1.25x a measured decision needs` rather than leaving a reader to
wonder why a number above 1.0 was refused.
