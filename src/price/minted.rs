//! The measured evidence: one [`Calibration`] row per (operating system, architecture,
//! kernel) triple anybody has actually timed, plus the fail-safe row for everyone else.
//!
//! Every row here is minted by `cargo run --release --example mint` over a large slice of
//! real source, each kernel timed alone as the minimum of several full traversals.
//! `.github/workflows/mint.yml` is that run on real hardware, for each of the six machines
//! `.github/workflows/native.yml` proves this crate correct on, and nine of the ten rows
//! below came out of a single dispatch of it against one 60.6 MiB corpus. That they share a
//! session matters for the reason four paragraphs down; the tenth is
//! [`MACOS_AARCH64_NEON`], which says why it is the tenth.
//!
//! These numbers state the whole economics of this crate:
//!
//! * the sieve beats the engine's per-byte walk by several times — a real
//!   advantage, and the reason any of this pays;
//! * but the engine's *skip* is still an order of magnitude faster than the
//!   sieve. Nothing that inspects every byte can front a `memchr`. That is not
//!   a defect in the kernel; it is the arithmetic that decides where it belongs.
//!
//! `dfa_excursion` is solved from a slate of lead bytes spanning two orders of
//! magnitude of frequency rather than assumed. Read at class resolution the
//! inverted values disagree by about tenfold; read from a per-byte table they
//! collapse into a narrow band — so that spread was the approximation talking,
//! and closing it is what makes a single coefficient defensible here.
//!
//! The one-conjunct slot is unmeasured because the lattice harvest fills to
//! [`MAX_CONJUNCTS`] whenever it yields anything at all, so no pattern on the mint's
//! slate reaches it. [`Calibration::sieve_per_byte`] extrapolates it conservatively
//! rather than treating the hole as free.
//!
//! The absolute per-byte figures — `dfa_skip`, `dfa_walk`, `sieve` — carry up to
//! double-digit-percent run-to-run variance on a loaded machine, because `mint`
//! times them unpaired. Because the gate is scale-invariant, that variance costs
//! no decisions: a run under load inflates them together.
//!
//! A re-mint is a fresh complete measurement, not a splice of old and new
//! afternoons: the gate reads *ratios* between these numbers, and a ratio built
//! from two different sessions is not a measurement of anything. The exceptions are
//! the two excursion coefficients, which are dimensionless and already
//! self-normalized inside a single interleaved timing window — `mint`'s `paired`
//! re-times both baselines against the pattern they divide, round by round — so
//! they may be carried forward when the rest of the row is re-taken. The higher of
//! consecutive paired mints is the one recorded, because an overstated excursion
//! can only decline a skip.
//!
//! Which figures move is itself evidence, and worth trusting over any single run.
//! Two independent mints of one machine, six days and one `regex-automata` removal
//! apart, re-derived its memory-resident `dfa_excursion` to within a fraction of a
//! percent while the unpaired figures beside it moved by several — the split falls
//! exactly where `paired` is and is not used. So an excursion coefficient that looks
//! like it drifted has almost certainly been read across machines or across regimes
//! rather than across afternoons.
//!
//! Two coefficients carry a [`Residency`](super::Residency) index because a
//! `memchr` and a dense-DFA re-entry are both cheaper once the bytes are already
//! resident; `dfa_walk` and `sieve` do not, because a dependent-load walk and an
//! issue-bound composition kernel have no headroom a hotter haystack could give
//! them. `skip_excursion` is indexed for symmetry and is not expected to move —
//! it re-enters sixteen blocks resident in either regime.
//!
//! # The mint can be fooled, and was
//!
//! A first residency mint read both columns identical, which looked like "residency
//! does not matter on this silicon". It was not a finding: `mint` was aimed at a
//! tree smaller than either requested working set, so both columns were the same
//! bytes timed twice. `examples/mint.rs` now refuses a corpus too small to leave
//! last-level cache rather than printing a row that says the memory system does
//! not exist — a row is a claim about a memory system, and a mint that never
//! reached memory has no business making one. The same trap makes a cache-resident
//! re-mint look like the shipped memory-resident row went stale.
//!
//! # Seven of these ten rows carry no cache-resident column
//!
//! Which is `mint` declining to paste a number it does not believe, not a measurement
//! nobody took. All six legs timed both regimes; on four of the six machines the
//! cache-resident column came back *worse* than the memory-resident one — a `memchr`
//! that got slower with the bytes already in cache, which describes a shared runner's
//! scheduler rather than a memory system. `examples/mint.rs` recognizes that inversion
//! and emits `0.0` across the whole column, which reads as unmeasured, so a caller
//! declaring [`Residency::Cache`](super::Residency) on one of those machines is
//! declined instead of being armed on the strength of noise. Two machines — `linux`
//! x86_64 and `macos` aarch64 — returned a coherent pair, and they are the two whose
//! rows have both.

use super::calibration::{Calibration, REGIMES};
use crate::lattice::MAX_CONJUNCTS;
use crate::shuffle::Kernel;

/// The aarch64 Linux server, and one of the three machines whose survey failure bought the
/// operating-system column: it had been arming on [`MACOS_AARCH64_NEON`]'s numbers and
/// losing to real source text by 8%.
pub const LINUX_AARCH64_NEON: Calibration = Calibration {
    os: "linux",
    arch: "aarch64",
    kernel: Kernel::Neon,
    host: "linux aarch64 · 4 logical cores · Neon kernel",
    minted: "2026-08-09",
    dfa_skip: [0.0, 0.036157],
    dfa_walk: 1.601672,
    dfa_excursion: [0.0, 11.407365],
    skip_excursion: [[0.0, 8.589079], [0.0, 5.912553]],
    sieve: [0.0, 0.399124],
};

/// The x86_64 Linux runner on its 32-byte shuffle — and, with [`LINUX_X86_64_SSSE3`] beside
/// it, the cleanest comparison in this slice: two kernels timed on one machine in one
/// session, which is the only arrangement where a difference between them is about the
/// kernels at all.
///
/// AVX2 buys 11% of the sieve over SSSE3 (0.325 against 0.366 ns/B) while the engine walk
/// each is weighed against differs by a tenth of a percent. Doubling the shuffle width does
/// not halve the cost, because the kernel is load-bound rather than issue-bound. That it
/// buys anything is what makes it worth dispatching to; that it buys 11% rather than 50% is
/// why the decision had to be measured instead of assumed from the width.
pub const LINUX_X86_64_AVX2: Calibration = Calibration {
    os: "linux",
    arch: "x86_64",
    kernel: Kernel::Avx2,
    host: "linux x86_64 · 4 logical cores · Avx2 kernel",
    minted: "2026-08-09",
    dfa_skip: [0.019787, 0.034197],
    dfa_walk: 1.876710,
    dfa_excursion: [11.667164, 12.118318],
    skip_excursion: [[8.428401, 9.691899], [5.767844, 5.659967]],
    sieve: [0.0, 0.325473],
};

/// The same machine's 16-byte shuffle, and one of the two rows in this slice that reached a
/// coherent cache-resident regime. A row for the narrower kernel is not a fallback: it is
/// what makes the comparison above a measurement rather than an assertion.
pub const LINUX_X86_64_SSSE3: Calibration = Calibration {
    os: "linux",
    arch: "x86_64",
    kernel: Kernel::Ssse3,
    host: "linux x86_64 · 4 logical cores · Ssse3 kernel",
    minted: "2026-08-09",
    dfa_skip: [0.022706, 0.034235],
    dfa_walk: 1.878684,
    dfa_excursion: [11.675108, 12.158681],
    skip_excursion: [[8.465241, 9.786254], [6.428453, 6.484189]],
    sieve: [0.0, 0.366377],
};

/// Apple silicon — and the one row here **not** taken from the mint run that produced the
/// other nine, because this key turned out to cover two machines that disagree.
///
/// `mint.yml`'s `macos-aarch64` leg priced its own runner in the same session as everything
/// else: `dfa_excursion` 14.00, `skip_excursion` 10.18, sieve 0.285 ns/B. Those numbers are
/// not wrong about that runner. They are wrong about a 16-core Apple laptop, and the
/// difference is not subtle — pasted here, `examples/survey.rs` on one armed
/// `(?-u)panic!\(` and measured it at **0.566x**, a sieve running 1.8x slower than the
/// engine it displaced. The row below declines that same pattern at 1.089x.
///
/// What separates them is one ratio. Every machine in that session read `skip_excursion`
/// 20–30% under `dfa_excursion`; this machine reads them within 1%, which says its
/// dense-DFA table stays resident where a 3-core runner's does not — a claim about a cache
/// hierarchy, and exactly the kind of claim `(os, arch)` cannot key on. So the honest
/// reading is not that one measurement is bad but that
/// [`MINTED`]'s key is coarser than the fact, which it already says of itself.
///
/// Given two rows for one key, the one that ships is the one that overstates the engine's
/// disadvantage least. That is the same reasoning that makes `mint` record the *higher* of
/// two paired excursions — an overstated excursion can only decline a skip — and the
/// asymmetry behind it is not aesthetic: an under-armed sieve costs a win nobody sees,
/// while an over-armed one ships a measured slowdown, which is the only one of the two
/// `survey` is willing to call a failure. This row is also the one that describes the
/// machines that exist in quantity; a 3-core virtualized runner is an artifact of CI, not a
/// population. It is green on both machines, which the runner's row is not.
pub const MACOS_AARCH64_NEON: Calibration = Calibration {
    os: "macos",
    arch: "aarch64",
    kernel: Kernel::Neon,
    host: "macos aarch64 · 16 logical cores · Neon kernel",
    minted: "2026-08-09",
    dfa_skip: [0.012390, 0.017507],
    dfa_walk: 1.313341,
    dfa_excursion: [8.057903, 9.751283],
    skip_excursion: [[7.611088, 9.647507], [7.788777, 6.963965]],
    sieve: [0.0, 0.196478],
};

/// The Intel Mac, which is the machine that made the operating-system column unavoidable.
///
/// Same instruction set as [`LINUX_X86_64_AVX2`], and not the same economics: its AVX2
/// sieve costs 0.527 ns/B where the Linux row it used to borrow reads 0.325 — a 62%
/// understatement — against an engine walk that differs by under 2%. Arming on the
/// difference between those two figures is exactly what `examples/survey.rs` caught losing.
pub const MACOS_X86_64_AVX2: Calibration = Calibration {
    os: "macos",
    arch: "x86_64",
    kernel: Kernel::Avx2,
    host: "macos x86_64 · 4 logical cores · Avx2 kernel",
    minted: "2026-08-09",
    dfa_skip: [0.0, 0.081309],
    dfa_walk: 1.910037,
    dfa_excursion: [0.0, 13.262007],
    skip_excursion: [[0.0, 10.880693], [0.0, 6.625020]],
    sieve: [0.0, 0.527253],
};

/// The Intel Mac's 16-byte shuffle, and the widest kernel gap any machine here reports:
/// 0.922 against AVX2's 0.527 ns/B, where the same two kernels on the Linux runner differ
/// by 11%. The ratio between two kernels is a property of the machine too, which is the
/// same finding one column over.
pub const MACOS_X86_64_SSSE3: Calibration = Calibration {
    os: "macos",
    arch: "x86_64",
    kernel: Kernel::Ssse3,
    host: "macos x86_64 · 4 logical cores · Ssse3 kernel",
    minted: "2026-08-09",
    dfa_skip: [0.0, 0.084880],
    dfa_walk: 2.136668,
    dfa_excursion: [0.0, 13.655867],
    skip_excursion: [[0.0, 12.677342], [0.0, 8.129890]],
    sieve: [0.0, 0.921573],
};

/// Windows on arm64, priced within 2% of [`LINUX_AARCH64_NEON`] on near-identical silicon.
/// Which is what the old two-column key predicted — and is a result rather than grounds for
/// having assumed it, since the two x86_64 operating systems disagree by 62%.
pub const WINDOWS_AARCH64_NEON: Calibration = Calibration {
    os: "windows",
    arch: "aarch64",
    kernel: Kernel::Neon,
    host: "windows aarch64 · 4 logical cores · Neon kernel",
    minted: "2026-08-09",
    dfa_skip: [0.0, 0.041957],
    dfa_walk: 1.622228,
    dfa_excursion: [0.0, 11.249269],
    skip_excursion: [[0.0, 7.890919], [0.0, 5.752287]],
    sieve: [0.0, 0.405517],
};

/// The first AVX-512 row this crate has ever had, and it prices the 64-byte kernel **slower
/// than the 32-byte one**: 0.458 ns/B against [`WINDOWS_X86_64_AVX2`]'s 0.376, on one
/// machine in one session, with the engine walk they are each weighed against differing by
/// a tenth of a percent.
///
/// So it is a result and not an artifact of two sessions, and it is the row that earns
/// [`crate::shuffle::kernel`] ranking by measured cost instead of by register width. Under
/// the width prior this machine would have elected AVX-512 and run 22% slower than the rung
/// beneath it, and no test in this crate would have objected, because AVX-512 is *correct*
/// here — `ci.yml` proves that under Intel SDE on every run. Correct and slower is precisely
/// the state a width prior cannot see.
///
/// Why it is slower is not settled, and this row does not need it to be. What has been ruled
/// out is the first explanation: an earlier mint read 0.335 against AVX2's 0.290 while both
/// kernels were spilling their four composition chains to the stack every step, so that
/// comparison measured store-to-load forwarding rather than shuffle throughput. Unrolled
/// into registers both got faster, and the ordering did not change.
pub const WINDOWS_X86_64_AVX512: Calibration = Calibration {
    os: "windows",
    arch: "x86_64",
    kernel: Kernel::Avx512,
    host: "windows x86_64 · 4 logical cores · Avx512 kernel",
    minted: "2026-08-09",
    dfa_skip: [0.0, 0.077105],
    dfa_walk: 1.753297,
    dfa_excursion: [0.0, 12.018238],
    skip_excursion: [[0.0, 8.701636], [0.0, 5.750183]],
    sieve: [0.0, 0.458317],
};

/// The kernel that actually wins dispatch on that machine, by being the cheapest of the
/// three it can run rather than the widest.
pub const WINDOWS_X86_64_AVX2: Calibration = Calibration {
    os: "windows",
    arch: "x86_64",
    kernel: Kernel::Avx2,
    host: "windows x86_64 · 4 logical cores · Avx2 kernel",
    minted: "2026-08-09",
    dfa_skip: [0.0, 0.067181],
    dfa_walk: 1.755121,
    dfa_excursion: [0.0, 11.968558],
    skip_excursion: [[0.0, 8.782805], [0.0, 6.404701]],
    sieve: [0.0, 0.375878],
};

/// And the 16-byte rung beneath it, at 0.437 ns/B — still cheaper than the 64-byte one two
/// rungs up, which is [`WINDOWS_X86_64_AVX512`]'s finding read from the other end of the
/// ladder.
pub const WINDOWS_X86_64_SSSE3: Calibration = Calibration {
    os: "windows",
    arch: "x86_64",
    kernel: Kernel::Ssse3,
    host: "windows x86_64 · 4 logical cores · Ssse3 kernel",
    minted: "2026-08-09",
    dfa_skip: [0.0, 0.069599],
    dfa_walk: 1.753784,
    dfa_excursion: [0.0, 12.136005],
    skip_excursion: [[0.0, 8.962550], [0.0, 7.320876]],
    sieve: [0.0, 0.436670],
};

/// The answer for a machine nobody has measured: **nothing is known**, so the sieve
/// price reads infinite and every pattern declines.
///
/// This is deliberately not a guess averaged from the rows above. The ratios are an
/// instruction-set property, and a target absent from [`MINTED`] is one whose
/// `memchr`, dependent-load walk, and byte shuffle stand in a relationship nobody has
/// timed — including, most sharply, a target with no byte shuffle at all, where the
/// sieve runs [`crate::shuffle::scalar`] and any vector-measured coefficient would be
/// pure optimism. Callers who would rather measure than decline can mint their own
/// and pass it in a [`crate::Policy`]; `cargo run --release --example mint` prints the
/// row.
pub const UNMEASURED: Calibration = Calibration {
    os: "unmeasured",
    arch: "unmeasured",
    kernel: Kernel::Scalar,
    host: "no machine — nothing here was measured",
    minted: "never",
    dfa_skip: [0.0; REGIMES],
    dfa_walk: 0.0,
    dfa_excursion: [0.0; REGIMES],
    skip_excursion: [[0.0; REGIMES]; 2],
    sieve: [0.0; MAX_CONJUNCTS],
};

/// Every (operating system, architecture, kernel) triple anybody has actually measured.
/// [`super::active`]
/// picks from here by matching the running target; adding silicon means adding a
/// row, not editing a default.
///
/// The key is `(os, architecture, kernel)`, and the first column is here because a
/// measurement put it here. It used to be `(architecture, kernel)`, on the reasoning
/// that Windows was not a third row waiting to be minted but a claim that the other two
/// already covered it — with the standing condition that an OS column "would only earn
/// its keep the day one leg measures a loss the other two don't".
///
/// That day came the first time `.github/workflows/native.yml` actually ran. Wired into
/// `ci.yml` rather than only `release.yml`, its six Windows/Linux/macOS × x86_64/arm64
/// legs put three machines on a row minted on a fourth, and `examples/survey.rs` caught
/// every one of the three arming a pattern that then lost against real source text —
/// `macos` x86_64 by 3%, both `aarch64` servers by 8%. The mint says why: that macOS box
/// times its own AVX2 sieve at 0.527 ns/B where the Linux row it was borrowing reads 0.325,
/// while the engine walk it is weighed against differs by under 2%. A row is a claim about
/// one machine's memory system, and the three legs that passed had been lucky in their
/// silicon rather than vindicated by it.
///
/// (`os`, `architecture`) is not the *right* key — the right key is the machine, and two
/// `aarch64` Linux servers can differ. It is the finest key a running binary can ask
/// about itself, which is a different and more useful property. See
/// [`OS`](crate::price::OS).
///
/// # This slice is also the dispatch ladder's permission list
///
/// [`crate::shuffle::kernel`] will not select a kernel that has no row here, so what
/// is *absent* from this slice is as load-bearing as what is present. Two consequences
/// worth stating out loud:
///
/// * A new instruction set lands without a flag day. A kernel is implemented,
///   differentially tested against the scalar reference on real silicon by
///   `tests/kernels.rs`, and **not dispatched to** until a row below was measured on it.
///   Adding one arms it; until then it moves no decision, and — the failure this ordering
///   exists to prevent — it cannot win a dispatch on a machine whose only calibration
///   describes a narrower shuffle and thereby strand that install on [`UNMEASURED`].
///   Which kernels are in that state is not left to be inferred from this slice's
///   absences: [`DORMANT`] names them, and a test holds the two lists to each other.
/// * A row is per *kernel* as well as per machine, which is why these names carry all
///   three parts of the key. One `cargo run --release --example mint` prints a row for
///   every kernel the running silicon has, so a machine's rows can be pasted in
///   together or one at a time without either one implying the other.
///   `.github/workflows/mint.yml` is that run, on real hardware, for each of the six
///   machines `.github/workflows/native.yml` proves this crate correct on.
///
/// And what this slice orders is nothing. [`crate::shuffle::kernel`] ranks the rungs a
/// machine has by the `sieve` cost its own rows report, so the *order* of the entries
/// here is presentation only — a slower kernel listed first cannot win a dispatch, and a
/// faster one listed last cannot lose it.
pub const MINTED: &[Calibration] = &[
    LINUX_AARCH64_NEON,
    LINUX_X86_64_AVX2,
    LINUX_X86_64_SSSE3,
    MACOS_AARCH64_NEON,
    MACOS_X86_64_AVX2,
    MACOS_X86_64_SSSE3,
    WINDOWS_AARCH64_NEON,
    WINDOWS_X86_64_AVX512,
    WINDOWS_X86_64_AVX2,
    WINDOWS_X86_64_SSSE3,
];

/// One kernel some silicon can execute that [`MINTED`] does not price *there*, and the
/// reason it is waiting.
///
/// Keyed on the machine and not only the kernel, for the same reason [`Calibration`] is:
/// one column short, it cannot express the state x86_64 is actually in. AVX-512 is priced
/// on `windows`/x86_64 and unpriced on `linux`/x86_64 — not from neglect, but because
/// GitHub's Linux fleet is *heterogeneous*, and the runner `mint.yml` drew had no AVX-512
/// while the runner `ci.yml` drew an hour later did. A kernel-only list has to call that
/// either priced (and then the Linux box that can run it is unaccounted for) or unpriced
/// (and then the Windows row that prices it is a contradiction). Both are false.
///
/// So `os` and `arch` say which machines an entry speaks for, and [`None`] means every one
/// of them.
#[derive(Debug, Clone, Copy)]
pub struct Dormant {
    /// The operating system this speaks for, or [`None`] for all of them.
    pub os: Option<&'static str>,
    /// The architecture this speaks for, or [`None`] for all of them.
    pub arch: Option<&'static str>,
    /// The kernel left unpriced.
    pub kernel: Kernel,
    /// Why it is unpriced, in the words of whoever left it that way.
    pub why: &'static str,
}

impl Dormant {
    /// Whether this entry speaks for the named machine.
    #[must_use]
    pub fn covers(&self, os: &str, arch: &str) -> bool {
        self.os.is_none_or(|it| it == os) && self.arch.is_none_or(|it| it == arch)
    }
}

/// Kernels this crate implements and some silicon can execute, that [`MINTED`] does not
/// price on that silicon — so [`crate::shuffle::kernel`] will not elect them there.
///
/// Empty is the goal, not the invariant. A kernel is written and differentially tested
/// long before anybody has an hour of the right silicon to price it on, and the
/// permission check in [`crate::arch::kernel`] exists precisely so that shipping it in
/// that state is safe rather than reckless. What this list adds is that the state has to
/// be **declared**, with the reason, by whoever leaves it that way.
///
/// The failure it catches is a quiet one, and it is the reason this is a list rather than
/// a paragraph. A kernel nobody has minted yet and a kernel that *stopped* being priced —
/// because a row was deleted, or because a wider rung was added above a priced one and
/// the mint was never re-run — are indistinguishable in [`MINTED`]: both are simply
/// absent. One is a plan and the other is a regression that costs every machine on that
/// silicon its throughput while every test still passes, since the narrower kernel is
/// still correct. Naming the first is what makes the second visible.
///
/// The test below holds this honest in both directions: a vector kernel this silicon can
/// run must be either priced or named here, and an entry that covers this machine must not
/// turn out to be priced on it after all. So a mint that lands a row also has to delete
/// its line.
pub const DORMANT: &[Dormant] = &[
    Dormant {
        os: None,
        arch: None,
        kernel: Kernel::Simd128,
        why: "the one kernel no `mint` leg can reach: a `wasm32` row's nanoseconds belong \
              to the runtime and the host under it, so the leg that prices one has to name \
              both, and `ci.yml` runs that target under `wasmtime` to prove the kernel \
              rather than to time it",
    },
    Dormant {
        os: Some("linux"),
        arch: Some("x86_64"),
        kernel: Kernel::Avx512,
        why: "the silicon, not the kernel: `mint.yml`'s `linux-x86_64` leg drew a runner \
              whose `available()` came back `[Avx2, Ssse3, Scalar]`, so there was no \
              AVX-512 to time. The fleet is not uniform — a `ci.yml` runner an hour later \
              reported the rung present — and which member a workflow is handed is not \
              something the workflow decides. `windows`/x86_64 got one that had it and is \
              priced; re-dispatching this leg until Linux draws the same is the only way \
              to fill it, and it would be elected only if it beat that machine's AVX2, \
              which on the machine that has both it does not",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::Residency;
    use crate::prior;

    /// Every row must describe a machine that could exist, and no two rows may claim
    /// the same (os, architecture, kernel) triple — [`super::super::active`] resolves by
    /// first match, so a duplicate would silently shadow a measurement.
    #[test]
    fn the_minted_rows_are_distinct_and_self_describing() {
        for (i, cal) in MINTED.iter().enumerate() {
            assert!(
                Residency::ALL.iter().any(|&at| cal.is_measured(at)),
                "{} {} row {i} measured nothing in any regime",
                cal.os,
                cal.arch
            );
            assert!(
                cal.kernel.is_vector(),
                "a scalar-kernel row would price the vector economics wrongly"
            );
            assert!(
                MINTED[..i]
                    .iter()
                    .all(|seen| (seen.os, seen.arch, seen.kernel) != (cal.os, cal.arch, cal.kernel)),
                "duplicate calibration for {} {} / {:?}",
                cal.os,
                cal.arch,
                cal.kernel
            );
        }
    }

    /// The safety property that lets an unpriced kernel exist in the tree at all:
    /// dispatch never elects one. Stated here rather than in [`crate::arch`] because it
    /// is a claim about *this slice* — the moment a row lands for a faster kernel the
    /// answer is allowed to change, and the moment one is deleted it must change back.
    #[test]
    fn dispatch_never_elects_a_kernel_this_slice_has_not_priced() {
        let chosen = crate::shuffle::kernel();
        let ladder = crate::shuffle::available();
        assert!(
            ladder.contains(&chosen),
            "dispatch chose {chosen:?}, which this silicon cannot execute"
        );
        let row = |kernel| {
            MINTED.iter().find(|c| {
                c.os == crate::price::OS && c.arch == crate::price::ARCH && c.kernel == kernel
            })
        };
        if ladder.iter().copied().any(|k| row(k).is_some()) {
            assert!(
                row(chosen).is_some(),
                "{} {} has a priced kernel but dispatch chose the unpriced {chosen:?}",
                crate::price::OS,
                crate::price::ARCH
            );
            // And the *cheapest measured* such, or the rows are decoration. Not the
            // widest: `available` orders by register width, which x86_64 has already
            // measured to be the wrong order, so this reproduces the rule in
            // `arch::kernel` rather than the prior it overrides.
            let best = ladder
                .iter()
                .copied()
                .filter_map(|k| row(k).map(|c| (k, c.sieve_per_byte(MAX_CONJUNCTS))))
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(k, _)| k);
            assert_eq!(
                Some(chosen),
                best,
                "dispatch settled for {chosen:?} while {best:?} is both cheaper and priced"
            );
        }
    }

    /// Every vector kernel this silicon can execute is either priced or **declared**
    /// dormant, and nothing declared dormant is secretly priced.
    ///
    /// The test above proves dispatch never elects an unpriced kernel, which is the
    /// safety property. This one is about the opposite hazard, which is not a safety
    /// property at all and is therefore the easier one to ship: a kernel that silently
    /// stops being dispatched to reads identically to one nobody has written yet, costs
    /// throughput rather than correctness, and breaks no other test in this crate. The
    /// only thing that can catch it is a list somebody has to edit.
    ///
    /// Scoped to machines that have *any* row, which the operating-system column made a
    /// distinction worth drawing. A machine nobody has minted at all is not a missing
    /// row — it is an unminted machine, it declines every pattern by design, and
    /// [`DORMANT`] is the wrong place to say so, since the kernel may be well priced on
    /// the next box over. Refusing to call that state a pass is `examples/survey.rs`'s
    /// job, and `.github/workflows/native.yml` runs it on all six legs. What is left here
    /// is the narrow, quiet failure this list exists for: a machine that *does* have rows
    /// and is missing one for a kernel its own silicon can run.
    #[test]
    fn an_unpriced_kernel_is_declared_rather_than_merely_absent() {
        let mine = || {
            MINTED
                .iter()
                .filter(|cal| cal.os == crate::price::OS && cal.arch == crate::price::ARCH)
        };
        let (os, arch) = (crate::price::OS, crate::price::ARCH);
        let priced = |kernel| mine().any(|cal| cal.kernel == kernel);
        let declared = |kernel| {
            DORMANT
                .iter()
                .any(|it| it.kernel == kernel && it.covers(os, arch))
        };
        if mine().next().is_some() {
            for &kernel in crate::shuffle::available() {
                assert!(
                    !kernel.is_vector() || priced(kernel) || declared(kernel),
                    "{os} {arch} has rows but none for {kernel:?}, which its silicon can \
                     run — mint one, or add it to DORMANT with the reason it is waiting"
                );
            }
        }
        for it in DORMANT.iter().filter(|it| it.covers(os, arch)) {
            assert!(
                !priced(it.kernel),
                "{:?} is priced on {os} {arch} but still listed DORMANT as {:?} — a row \
                 landing is what deletes that line",
                it.kernel,
                it.why
            );
        }
    }

    /// The measurement that decides where this crate is useful: the engine's skip is
    /// an order of magnitude faster than the sieve, and its walk is slower. Asserted
    /// of **every** minted machine and every regime it claims, so new silicon either
    /// reproduces the bracket or says out loud that the economics there are different.
    ///
    /// The bracket is what makes residency a dimension rather than a rescaling. It has
    /// to hold in both regimes — a `memchr` that ever became *slower* than the
    /// composition kernel would invert the whole cost model — and yet the width of it
    /// is exactly what a regime changes, since the skip end moves with the memory system
    /// while both other ends do not.
    #[test]
    fn every_minted_machine_brackets_the_sieve_between_skip_and_walk() {
        let mut swept = 0;
        for cal in MINTED {
            let sieve = cal.sieve_per_byte(MAX_CONJUNCTS);
            let (arch, host) = (cal.arch, cal.host);
            assert!(
                sieve < cal.dfa_walk,
                "{arch}: but it does beat a per-byte walk ({host})"
            );
            for at in Residency::ALL {
                if !cal.is_measured(at) {
                    continue;
                }
                swept += 1;
                assert!(
                    cal.dfa_skip[at as usize] < sieve,
                    "{arch} in {at:?}: no per-byte filter can front a memchr ({host})"
                );
            }
        }
        assert!(swept > 0, "no minted row claimed any regime");
    }

    /// A rare lead byte makes the engine unbeatable and a common one makes it barely
    /// better than walking. That ordering is the entire content of the escape-set
    /// model, so it is worth pinning independently of the coefficients — and on every
    /// machine, since it is the ordering that makes anything ever decline.
    #[test]
    fn a_rarer_accelerator_prices_the_rival_cheaper() {
        let freq = prior::Prior::Source.byte_freq();
        for cal in MINTED {
            for at in Residency::ALL {
                if !cal.is_measured(at) {
                    continue;
                }
                let rare = cal.rival_per_byte(b"W", &freq, at);
                let common = cal.rival_per_byte(b"e", &freq, at);
                let none = cal.rival_per_byte(b"", &freq, at);
                let arch = cal.arch;
                assert!(
                    rare < common,
                    "{arch} in {at:?}: a rare escape byte is a cheaper engine: {rare} vs {common}"
                );
                assert!(
                    common <= none,
                    "{arch} in {at:?}: no accelerator can cost more than plain walking"
                );
                assert!(
                    rare < cal.sieve_per_byte(MAX_CONJUNCTS),
                    "{arch} in {at:?}: a rare-anchored engine must out-price the sieve, \
                     or nothing declines"
                );
            }
        }
    }

    /// The direction the regime moves things, pinned so a re-mint cannot quietly invert
    /// it: a cache-resident haystack makes the *engine* cheaper, never the sieve.
    ///
    /// This is the finding that forced the dimension, and it is worth an assertion
    /// rather than only a paragraph. Both regime-indexed coefficients describe reaching
    /// memory — a `memchr` stream and a dense-DFA re-entry — so a row where either grew
    /// on the way into cache would be a row measured through something other than the
    /// memory system, which is the most likely way for a future mint to be wrong.
    #[test]
    fn a_cache_resident_haystack_only_ever_cheapens_the_engine() {
        let (cache, memory) = (Residency::Cache as usize, Residency::Memory as usize);
        let mut compared = 0;
        for cal in MINTED {
            if !(cal.is_measured(Residency::Cache) && cal.is_measured(Residency::Memory)) {
                continue;
            }
            compared += 1;
            let arch = cal.arch;
            assert!(
                cal.dfa_skip[cache] <= cal.dfa_skip[memory],
                "{arch}: a memchr cannot be slower with the bytes already in cache"
            );
            assert!(
                cal.dfa_excursion[cache] <= cal.dfa_excursion[memory],
                "{arch}: re-entering a resident DFA cannot cost more than a cold one"
            );
            // `skip_excursion` is deliberately **not** held to this. Both columns of it
            // re-enter a sixteen-block quotient that is resident in either regime, so
            // the physics predicts no gap — and it is minted as a *maximum* over a
            // five-pattern slate, which is a noisy statistic by construction. Measured,
            // instrument 1 comes out 7.79 in cache against 6.96 in memory: an inversion,
            // and one well inside the spread of the coefficient it belongs to. Asserting
            // a direction there would be asserting the absence of noise.
        }
        assert!(
            compared > 0,
            "no row holds both regimes yet — nothing above was actually compared"
        );
    }
}
