The gate can now be told that a survivor costs something other than a regex scan.
`Policy::rival` takes a `Rival`, which is `Rival::Engine` by default — read the price off
the automaton, exactly as before — or `Rival::Walks`, a dimensionless multiple of this
machine's dense-DFA walk, or `Rival::NanosPerByte` for a confirm somebody has actually
timed. `Walks` is the one to prefer, because a ratio rescales with the row it is a ratio
of and so `scaling_the_whole_calibration_changes_no_decision` keeps holding; a duration
does not, which is asserted rather than admitted so the caveat stays measurable.

This was the crate's largest unreachable audience. A refutation's product is a proof that
a document needs no further work, and what that is worth is set by what the work would
have been — but the price could only be read off a `Dfa`, which describes what the
*pattern* costs to confirm and not what the caller's pipeline costs to run. A caller could
only forge a `Calibration` and misuse `dfa_walk` to mean something it does not, corrupting
`skip_per_byte` in the same motion. `examples/census.rs` now sweeps the same 31-pattern
population against a document extraction as well as against the engine.

A costly rival is nonetheless inert on its own, and `Policy::bypass` is the term that says
why — see *A baseline the caller would really have run*. An expensive rival divides the
pre-pass and cannot manufacture selectivity either: as the price grows the gate converges
on `survival * (1 + MARGIN) < 1`, the same limit `Policy::rivals` converges on for the same
reason, so a filter that retires nothing is unrescuable at any price and `tests/rival.rs`
holds it there. The documentation also names the confirms that do *not* qualify, which is
the more useful half: a walk is 1.3–2.1 ns/byte on the minted machines, so gzip, AES with
hardware support, and JSON parsing are all within a small multiple of the engine and
change nothing.

`Residency::of_working_set` turns a byte count into a regime against the new
`price::RESIDENT_ABOVE`. The residency question stays the caller's, because this crate
still cannot see the corpus — but a caller who knows how many bytes they are about to hand
the engine was being asked to answer it twice, and `examples/survey.rs` had the arithmetic
copied into it. The one way it can be wrong is named on the function: a re-scanned working
set is cache-resident however large, and that error arms rather than declines.
