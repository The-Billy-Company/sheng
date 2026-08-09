A `Dfa` trait, and with it a `no_std` build: `--no-default-features` is now a crate
whose only dependency is `memchr` and whose only requirement is an allocator.

This was never a port so much as a boundary finally drawn where the code already
divided. Nothing in `regex-automata` was ever on the scan path — it parses a pattern
and hands over a `dense::DFA`, and after that the sieve is arithmetic over sixteen
bytes of table. The projection asked exactly six questions of that automaton, so those
six are now the `Dfa` trait, `regex-automata`'s dense DFA is one implementor of it, and
`Sieve::of_dfa` is generic over the rest. A hand-written transition table, a
zero-copy deserialized automaton, or an engine behind an FFI boundary can all drive
the whole pipeline now, and `tests/dfa.rs` proves it by doing so from four states and
three byte classes with no engine anywhere in the file.

That also closes a semver leak nobody had tripped on yet: `Sieve::of_dfa` and
`Projection::of` used to name `regex_automata::dfa::dense::DFA<Vec<u32>>` in their
signatures, which made a major version of somebody else's crate a major version of
this one.

Getting to `no_std` then cost less than expected, because what it removed was mostly
`std` being asked for things `core` already knew:

- `f64::powf`, the crate's only transcendental, was the survival term's `(1-f)^len`.
  The exponent is a **count of bytes**, so exponentiation by squaring is the whole
  operation — held to `powf` across nine lengths and nine rates in a test that runs
  wherever there is a `std` to disagree with. Every float operation in the crate is now
  `+ - * /` and a comparison. No `libm`, no math library behind it.
- `std::env::consts::ARCH` is a per-target compile-time constant that only looked like
  it needed an operating system to ask. `price::ARCH` reads it from `cfg` and is
  checked equal to `std`'s.
- `std::arch::is_x86_feature_detected!("ssse3")` is now one `CPUID` leaf read, memoized
  in an `AtomicU8`. This is the case where hand-rolling a feature probe is *equal* to
  `std`'s rather than weaker than it: SSSE3 is a plain `CPUID.01H:ECX[9]` bit with no
  operating-system participation, unlike the AVX-512 family where a set bit only means
  the silicon can and `XGETBV` has to be asked whether the kernel will. One
  implementation for both configurations, not a `cfg` with two answers.
- Two `HashMap`s in the projection became a sorted `Vec` and one reused scratch column.
  This was supposed to be a lateral move to drop a `std` type and to ask only `Ord` of a
  caller's state type instead of `Hash` plus a hasher. It is **2.5-3x faster**
  (`cargo run --release --example bench`, `project µs` over the eight-pattern slate,
  three runs each): 32→13, 31→12, 27→10, 24→8.5, 21→7.7, 14.4→4.1, 22.8→8.0, 23.1→7.9.
  The reachable core is capped at 96 states, where seven compares beat hashing an opaque
  key — and the class refinement had been allocating 256 vectors and hashing up to 192
  bytes per byte value to discover that most of them were duplicates.

`memchr` stays, unconditionally and deliberately. It is `no_std`-capable already, it is
on the scan path for 1-to-3-byte escape sets, and nothing hand-rolled here would match
its tuning at that width — so the `std` feature now just elects its AVX2 runtime
dispatch instead of the crate insisting on it.

Two things fell out of building for `x86_64-unknown-none`, which is soft-float and has
no SSE at all: the SSSE3 kernels are now gated on `target_feature = "sse2"` rather than
on the architecture alone (without it a 128-bit vector cannot be held, and the code
generator says so), and such a target honestly reports `Kernel::Scalar`. CI checks the
complete powerset of the two features, both bare-metal targets, and runs
`tests/dfa.rs` against the library with its dependencies and its `std` removed — because
compiling is not running.
