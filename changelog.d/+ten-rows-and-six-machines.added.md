`price::MINTED` goes from two rows to ten, and every kernel this crate implements for real
silicon is now priced on at least one machine it runs on. `.github/workflows/mint.yml` was
dispatched across all six legs `native.yml` proves correctness on — Linux, macOS and Windows
on x86_64 and arm64 — against one 60.6 MiB corpus in one session, which is the only
arrangement in which these numbers may be divided by each other.

The consequences are dispatch, not decoration. x86_64 machines stop running the 16-byte SSSE3
path because AVX2 was implemented but unpriced: on the Linux runner AVX2 costs 0.325 ns/B
against SSSE3's 0.366, and on the Intel Mac 0.527 against 0.922. `price::DORMANT` is down
from three entries to one — AVX2 and AVX-512 both have rows, and `Simd128` remains the only
kernel no mint leg can reach, because a `wasm32` row's nanoseconds belong to the runtime and
the host under it.

## AVX-512 is priced, and loses

The first AVX-512 row this crate has ever had reads 0.458 ns/B where the same machine's AVX2
reads 0.376 and its SSSE3 reads 0.437 — the 64-byte shuffle is the slowest of the three rungs
on the only machine that offers all three. That is the row cost-ordered dispatch exists for:
AVX-512 is *correct* here, proven under Intel SDE on every CI run, and correct-but-slower is
exactly what a register-width prior cannot see. It is priced and simply not elected.

## Seven of the ten rows carry no cache-resident column

All six legs timed both regimes; four came back with a cache-resident column *worse* than the
memory-resident one, which describes a shared runner's scheduler rather than a memory system.
`examples/mint.rs` detects the inversion and emits the whole column as `0.0`, so those
machines decline a caller who declares `Residency::Cache` instead of arming on noise.

## One key, two machines that disagree

`macos`/`aarch64` is the row that did not come from this session, and the reason is worth
stating rather than hiding. Its mint leg priced a 3-core runner at `dfa_excursion` 14.00
against `skip_excursion` 10.18; pasted in, `examples/survey.rs` on a 16-core Apple laptop
armed `(?-u)panic!\(` and measured the result at 0.566x — a sieve running 1.8x slower than
the engine it displaced. Every machine in the session read those two coefficients 20–30%
apart; that laptop reads them within 1%, which is a claim about a cache hierarchy and
precisely what `(os, arch)` cannot key on.

So the shipped row is the laptop's, on the rule that when one key covers machines that
disagree, the row that ships is the one overstating the engine's disadvantage least — the same
asymmetry that makes `mint` record the *higher* of two paired excursions. An under-armed sieve
costs a win nobody sees; an over-armed one ships a measured slowdown, and only the second is a
failure `survey` will name.
