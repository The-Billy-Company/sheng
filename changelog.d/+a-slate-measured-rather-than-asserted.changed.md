The rule-slate case — one refutation retiring many searches — was the workload this crate
named as its best and never measured. `tests/slate.rs` now measures it, and the walls it
found are documented where a caller plans against them rather than discovered after.

**If the rules carry literals, the union already is the answer.** Sixty-four
literal-prefixed rules measure 11.96 ns/B as separate engines and 0.12 as one union — the
fan-out almost exactly, because the union keeps a multi-literal accelerator and still pays
one pass's price. A sieve in front of that retires nothing, and `Bypass::Slate` is how the
gate is told so. What bounds the union is construction rather than throughput, which
`Bypass::Slate` now states in figures: 12.6 KiB, 4.5 MiB and 65 MiB of dense table at 1, 64
and 256 rules, builds of 0.2 ms, 0.75 s and 114 s, and no determinization at all past 256
inside a gibibyte.

**A slate's own union stops being sieveable almost immediately.** Over eight literal-free
rules of the kind a secret scanner is made of, the union's reachable core passes
`MAX_CORE_STATES` by the seventh and the lattice stops finding a register-sized closed
partition at the *second*. A 16-block quotient of 1,200 rules is not a filter that lost on
price; it is a filter that does not exist. What does exist is a coarse skeleton of one
*family* — `[0-9]+[-./:][0-9]+[-./:][0-9]+` contains every SSN, card number, date,
timestamp and version string in nine states — which is why `Sieve::of_superset_with` takes
an automaton and not a pattern list. The test is written as a search asserted in the
direction that catches the claim getting *better*, so a lattice that ever harvests wider
fails it and the prose it justifies has to be rewritten.

**Every slate size converges on the same ceiling**, so record length rather than rule count
is what decides. That skeleton is worth at most 11.3x over 256-byte records, 3.2x over a
kilobyte and 1.00x over 16 KiB — the same filter, the same slate, three different answers.
The slate regime is packets and log lines; over large documents there is nothing to win.

`examples/bench.rs` now times the engine beside the kernel across record lengths instead of
the kernel alone, which is what `VALIDITY_FLOOR` is actually a claim about — the gate
compares a ratio, and the sieve's own curve cannot settle where a verdict stops travelling.
Swept, the sieve's edge over a walking rival holds within a couple of percent from 64 KiB
down to a kilobyte, is 16% under nominal at 256 bytes, and 39% under at 128: the floor sits
exactly where the model's error crosses `MARGIN`, measured rather than argued.

That sweep also retired the fix it was expected to justify. Minting per-call constants
would not have lowered the floor, because the larger short-record effect runs the other
way: consecutive searches over short records are independent dependency chains that a wide
core overlaps, so the *rival* gets cheaper per byte — 1.27 ns/B over 4 KiB records against
0.71 over 64-byte ones. That is a reorder window saturating rather than a coefficient, and
a number fitted to one machine's window describes no other machine's. A floor states the
same fact without claiming a portability it does not have.
