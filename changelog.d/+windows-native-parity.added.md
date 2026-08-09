Windows joins Linux and macOS as an equally first-class target, on both x86_64 and
arm64. Nothing in `src/arch/` changed to get there - it already dispatched on
`target_arch` alone, never on the operating system, so the same NEON and SSSE3
kernels this crate already shipped simply had never been proved running natively
under Windows.

They now are. `.github/workflows/native.yml` runs the full soundness differential,
the kernel-agreement census, and the `survey` economic gate on real native silicon
across all six Windows/Linux/macOS × x86_64/arm64 targets on every push - never
emulated, never cross-compiled - and gates `cargo publish` on all six passing
uncached. The matrix exists to keep proving that a row's numbers hold on the machine
running them rather than to assume it, and it collected inside this same release: the
six cells disagreed, so `price::MINTED` is now keyed by `(os, architecture, kernel)`
and carries ten rows rather than two. A target still absent from the matrix declines
with `BuildError::Uncalibrated` instead of inheriting another platform's numbers.
