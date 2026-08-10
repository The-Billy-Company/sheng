A machine with no row in `price::MINTED` can now measure its own. `Calibration::measure`
takes a sample of the caller's documents and hands back a row; `price::Bench` is the same
measurement with the knobs, and `Bench::report` adds every solution behind every solved
coefficient. The row goes into `Policy::calibration` and the gate starts deciding.

`riscv64`, `powerpc64`, `s390x` and `loongarch64` were **inert**, and the reason was worth
naming precisely because it was not portability: every kernel in this crate compiles and
runs there through `Kernel::Scalar`, which needs no vector instruction at all. What those
machines lacked was five coefficients, and the only way to get them was to clone this
repository, run an example, paste a `const`, open a pull request and wait for a release. So
the whole audience on unminted silicon was refused for want of a measurement their own
machine was standing there able to take.

A runtime row is *better* evidence than a shipped one on two counts. It describes this
machine rather than one sharing its `(os, arch, kernel)` triple — which cannot tell an
M-series laptop from a datacenter Ampere — and it is taken over the caller's own bytes, so
the marginals every escape set is priced under are measured rather than borrowed from one of
four shipped corpora. What it gives up is reproducibility, and the mitigation is the mint's:
a minimum over several traversals, with every ratio's legs interleaved so contention falls
on numerator and denominator alike. `price::histogram` is public for the same reason, since a
caller measuring their own corpus wants `Policy::freq` to describe it too.

Every refusal in `Unmeasurable` is a case where a row could have been returned and would have
been fiction. A sample under `MEASURABLE_ABOVE` is refused because at a few kilobytes the
clock's granularity *is* the measurement; a sample holding the `\x00\x01zz` sentinel is
refused because the reference patterns would match and time an early exit rather than a
traversal. One sample is one memory regime, so a row fills the column its own byte count
earns and leaves the other reading unmeasured — `Calibration::regime` names which, and
`Calibration::merge` combines a cache-sized sweep with a memory-sized one, refusing a pair
whose cache column prices the engine's skip *above* its memory column, since a hotter
haystack cannot cost more and such a pair measured a busy machine.

`examples/mint.rs` now calls all of this instead of carrying its own copy, and is what it
always should have been: the *publishing* half. It still prints the pasteable `const`, the
two fields a runtime row cannot fill (`host` and `minted` are `&'static str`), the per-kernel
sweep through `shuffle::force`, and the spread beside every mean. Three hundred lines of
timing loops, reference patterns and inversions left it. The duplication was a live drift
hazard rather than untidiness: the slate a `sieve` coefficient is timed over and the slate a
skip is judged against are supposed to be the same slate, and with two copies nothing said
so. `BuildError::Uncalibrated` now names the remedy, since it is the only variant in that
enum a caller can lift from where they stand.
