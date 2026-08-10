`Screen` is a matcher that uses a sieve when one pays and the engine alone when one does
not. `Screen::new` returns an error only for a pattern that does not parse; every economic
and structural refusal is absorbed, and `is_match` answers exactly what `regex-automata`
answers either way.

`Sieve` hands back a `Result` whose *common* variant is refusal, and that is honest — most
patterns should not be fronted by a filter. But it made the ordinary outcome of
`cargo add sheng` an error a caller has to write code around, for a speedup they then do not
get, and the rational response to that is to remove the dependency. Which is the wrong
outcome twice over, because the decline is a fact about **this** pattern on **this** machine
over documents of **this** length, and every one of those changes without the caller doing
anything: a slate grows, a machine gets a row, a corpus moves from cache to memory. A caller
who removed the crate never finds out.

Nothing is hidden by absorbing it. `Screen::sieve` hands over the sieve and its arithmetic,
`Screen::declined` hands over the refusal verbatim, `Screen::armed` says which of the two
happened, and `Screen::dfa` exposes the automaton for a caller who needs a position rather
than an existence answer. What is removed is the obligation to *handle* it.

One automaton serves both roles, which is the arrangement `Sieve::of_dfa` recommends and
this type makes automatic: the sieve is priced against the very engine that will confirm its
survivors, so the gate's rival term measures the search that is really going to run rather
than one like it. `tests/screen.rs` is mostly a single differential against the engine over
synthetic bytes drawn from each pattern's own alphabet and over real source text, since a
type whose whole purpose is to be indistinguishable from the engine is worth exactly what
that indistinguishability is worth — and it separately asserts that an armed screen really
refutes, because the differential would pass just as happily with the sieve wired to
`false`.
