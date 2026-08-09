Dispatch elects the kernel this machine's own rows price *cheapest*, where it used to
elect the widest kernel that had a row at all. Register width was standing in for speed,
and x86_64 has now refuted the substitution: a mint on a four-core Windows runner timed the
AVX-512 composition pass at 0.458 ns/B against AVX2's 0.376 and SSSE3's 0.437, so the
64-byte shuffle lost to both narrower rungs on the machine that has it. Under the old rule,
landing that row would have moved every such machine onto the *slowest* of the three, and
the row proving it slowest is the row that would have done it.

`arch::available` still returns the ladder widest-first and still ends in `Scalar`, but
that order is now explicitly a prior: it decides nothing where a measurement exists,
breaks ties where two rows agree, and answers on a machine nothing has priced. What ranks
the rungs is `sieve_per_byte` at `MAX_CONJUNCTS`, read from `price::MINTED` for the
running `(os, arch, kernel)`. `minted.rs`'s dispatch test asserts the same rule rather
than the width order it used to assert, so a wider-but-slower kernel cannot win a dispatch
without failing the build.

This also removes the last reason to keep a measured kernel dormant, and `price::DORMANT` is
down to one entry because of it. A kernel that loses on the machine that measured it can now
be priced and simply not chosen there, which is a fact in `MINTED` rather than an absence
from it — and on a machine where it wins, it wins.
