The gate can now be told how many searches one refutation lets a caller skip.
`Policy::rivals` is one by default — one pattern, one engine, one document, which is the
shape the arithmetic was written for — and above one it makes a **slate** a different
economic proposition from a pattern rather than the same one repeated. The pre-pass is paid
once per document while verification is paid once per rival, so the inequality divides
through to `(sieve/rivals + survival * rival) * (1 + MARGIN) < rival` and the sieve's own
price stops being what declines a near-parity filter.

That term was the difference between the workload this crate is theoretically best at and
the one it could actually serve. A secret scanner, a log triage rule set, a classifier — all
of them run tens of patterns over each document, and every one of them was being priced as
though a refutation saved a single search. `src/price/gate.rs` carries the unit tests for
what the term may and may not do: it is monotone in `rivals`, it amortizes the pre-pass and
*only* the pre-pass, and it converges on `survival * (1 + MARGIN) < 1` — so a filter that
retires nothing is unrescuable at any fan-out, which is the same ceiling `Rival` converges
on for the same reason. `tests/slate.rs` finds the crossing on a real pattern pair and also
keeps an unrescuable one, so the limit is exercised rather than described.

Two obligations come with it and neither is checkable from inside the crate, so both are
stated on the field. One refutation really must skip them all, which means the sieve has to
come from an automaton whose language contains every pattern's — `Sieve::of_superset_with`
is the direct way, and a union automaton the direct source. And the priced rival should be
the *cheapest* of the slate, since the engines in a real slate differ by an order of
magnitude and underestimating the rival can only make the sieve decline.

`BuildError::NotWorthIt` names the fan-out in its message when there is one, and says
nothing when there is not — so a single-pattern decline reads exactly as it did, while a
caller who declared a slate and still declined can see the term was applied and was not
enough rather than wondering whether it was read at all.
