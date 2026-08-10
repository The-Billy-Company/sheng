The gate no longer prices a sieve against doing nothing. `Policy::bypass` takes a `Bypass`
— `Engines` by default, or `Slate(Rival)` for a caller who can name the whole alternative,
or `Absent` for one who has none — and `CostFact::unfiltered` is now the **cheaper** of the
rival slate and that bypass.

This closes the crate's largest arithmetic hole, and it is worth stating as the objection
it answers. `Rival::Walks(512.0)` armed 24 of 31 census patterns, and that number was a
comparison against a pipeline nobody runs. If a survivor costs 512 walks a byte then a walk
costs 0.2% of a survivor, so a caller holding a regex would put the engine in front of the
extraction — an *exact* filter, whose survival rate is the true hit rate rather than the
sieve's fallthrough. At a secret scanner's ~1e-4 that alternative costs about 1.05 walks a
byte against the sieve's ~410, and it wins by two orders of magnitude. The gate compared
the sieve against no filter and never against the cheap exact one already in the dependency
graph, so the regime where most patterns armed was the regime where arming was mostly
wrong.

Measured with the term in place, `examples/census.rs` now reports 11 of 31 arming in front
of the engine and **the same 11** in front of the 512-walk confirm, because the engine is
still there to run first. Only `Bypass::Absent` reaches 24, and that is the honest value of
a costly rival: it belongs to a caller who genuinely cannot decide the question more
cheaply where the sieve runs — screening packets against rules whose matches only exist in
a reassembled flow, not "my confirm is slow". `tests/rival.rs` asserts all four corners,
including that a nonsense price still cannot arm anything through the new term.

`CostFact::ceiling` is the companion, and `BuildError::NotWorthIt` now prints it. Every
amortizing term converges on `1 / survival`, so the ceiling is the most any rival price or
slate size could ever have reached, and a decline under the margin at its own ceiling is
not arguable. Six of the seven census declines are terminal in exactly that sense — which
the summary now separates rather than reporting one number for two findings.
