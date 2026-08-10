Every claim this project makes now has one home, and the copies that used to restate it
point at that home instead. The soundness argument, the arming inequality, the start-block
skip's exactness, the operating-system column in `price::MINTED`, and the prior-art
bibliography were each told in full in three or four places — README, crate docs, a module
`//!`, a nested `README.md` — which is how they came to disagree. The crate-root
documentation is roughly half its previous length and keeps the pitch, the compiled usage
example, and pointers into the modules that own the contracts; the nested `src/**/README.md`
files are filesystem maps again rather than second Design essays.

The disagreements the duplication had already produced are gone with it. `price::MINTED`
was described as keyed on `(architecture, kernel)` in the crate docs and on the triple
everywhere else; `SECURITY.md` asked reporters for a `Policy::default()` that no longer
exists, `Policy::new(residency)` having replaced it; `examples/mint.rs` still called a price
row a claim about a pair. Absolute nanoseconds-per-byte figures appeared in four documents
at two different pairs of values, so they now appear only beside the `Calibration` constants
they annotate, where a re-mint edits them in the same motion it edits the number.

What is left in prose is checkable or named. The README cites `MAX_CONJUNCTS` rather than
the digit two, and a compile-time assertion in `lattice.rs` holds that constant and `LANES`
to the sixteen-lane, two-quotient shape the prose describes, so moving either becomes a
build failure rather than a silent falsehood. This is the guard the `#[cfg(doctest)]` README
include already provides for the Rust blocks, applied to the two numbers that were most
exposed without one.
