`price::MINTED` is keyed on `(os, architecture, kernel)`. It was keyed on the pair, on
the argument that an OS column would be warranted "the day some target's own measurement
disagrees, not before". That day arrived the first time `.github/workflows/native.yml`
ran on every push rather than only before a publish.

Three of its six legs were pricing themselves from a row minted on a fourth machine, and
`examples/survey.rs` caught every one of the three arming a pattern that then lost
against real source text - macOS x86_64 by 3%, both `aarch64` servers by 8%. The mint says
why: that macOS box times its own SSSE3 sieve at 0.54 ns/B where the Linux row it was
borrowing claims 0.22, while the DFA walk the sieve is weighed against differs by only
half as much, so the ratio that decides arming moves with the machine. The instruction
set fixes what the kernel *is*; the cache hierarchy fixes what it costs against its
rival. The three legs that passed had been lucky in their silicon rather than vindicated
by it.

`Calibration` therefore gains an `os` field and `BuildError::Uncalibrated` an `os`
member - the decline has to name all three parts, because `MINTED` holding a row for your
architecture *and* your kernel and still not pricing your machine is now the ordinary
case rather than a bug in resolution. `price::OS` is the string to name, and it
enumerates only the five operating systems this repository builds: an OS nobody has
minted on reads `"unknown"`, matches nothing, and declines, which is what an unenumerated
one already did.

`examples/mint.rs` prints the column, and now withholds one it could not measure. A busy
runner that times a cache-resident haystack as costing the engine *more* than a
memory-resident one has measured a loaded machine rather than a memory system, so the
whole cache column is emitted as `0.0` - callers declaring `Residency::Cache` on those
rows get `Uncalibrated`. The whole column, not the inverted coefficient: `is_measured`
reads `dfa_skip` alone, so zeroing a lone inverted `dfa_excursion` would leave the regime
looking measured while pricing the engine's excursion at free, which is an over-arming
row.
