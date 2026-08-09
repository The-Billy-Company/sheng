`Sieve`'s documentation has always told callers that one immutable instance serves
every document and every worker, and until now nothing but auto-trait inference kept
that true. `Send` and `Sync` are derived from a type's fields, which is exactly what
makes them possible to lose in silence: one `Rc` handle, one `Cell` memoizing a probe,
one raw pointer into a mapped table, and the promise is gone - with the breakage
landing on the caller who believed the documentation rather than on the commit that
took it away. Every public type is now named in a compile-time assertion, so that
failure lands where it is caused.

`Sieve` and `BuildError` are held to `'static` on top of it, because they are the two a
caller moves *into* a worker rather than merely shares with one - a sieve built once and
sent to a pool, an error carried back across a join - and neither could do that while
borrowing from the automaton it was built from. The assertion is a `const` item rather
than a test, for the same reason the `no_std` job builds bare-metal targets: the
guarantee is not conditional on a test build. It is checked in all four feature
combinations and on every target this crate compiles for, including the ones with no
harness to run a test with and no threads to spawn.
