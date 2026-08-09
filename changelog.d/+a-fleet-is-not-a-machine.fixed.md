`price::DORMANT` is keyed on `(os, arch, kernel)` instead of on the kernel alone, because
one column short it could not describe the state x86_64 is in. Its entries are now a
`price::Dormant` struct whose `os` and `arch` are `Option`s, where `None` means every
machine.

GitHub's Linux x86_64 fleet is not uniform. `mint.yml`'s `linux-x86_64` leg drew a runner
whose `available()` came back `[Avx2, Ssse3, Scalar]`, and an hour later a `ci.yml` runner
on the same label reported AVX-512 present — so `linux`/`x86_64` is simultaneously a
machine that can run the kernel and a machine no mint has priced it on, while
`windows`/`x86_64` prices it outright. A kernel-only list has to call AVX-512 either priced,
leaving the Linux box unaccounted for, or unpriced, contradicting the Windows row. Both are
false, and the per-machine audit failed on exactly that.

The audit reads the machine on both sides now: a vector kernel this silicon can run must be
priced or covered by an entry that speaks for this machine, and an entry that speaks for
this machine must not turn out to be priced on it. Landing a row still deletes a line.

`tests/policy.rs`'s coinciding-accelerator test also stops asserting a decline flatly.
Where the sieve's leaves are the engine's own accelerator set the streaming halves of the
two prices cancel, leaving the verdict on the ratio of two excursion coefficients — 3.5%
apart on the one machine the assertion was written against, and 20–30% apart on five of the
six minted since. The margin cannot dismiss that and should not, so the test now requires
the decline *unless* this machine's own pair clears `MARGIN`, and still fails an arming that
rests on coefficients only noise separates. `windows`/`aarch64` arms, and
`examples/survey.rs` is what adjudicates it against real source.
