//! Every kernel this silicon can run, held to the scalar reference — not just the one
//! dispatch elected.
//!
//! Its own test binary, and that is the whole design of this file. `arch::force` moves
//! a process-wide decision, so a kernel sweep sharing a binary with any other test
//! would be changing what that test dispatches to underneath it, in parallel. Cargo
//! gives each integration file its own process, so one test per file is the isolation
//! and no lock is needed.
//!
//! # Why dispatch alone is not enough to differentiate a kernel
//!
//! `arch::kernel` returns the fastest kernel `price::MINTED` has a row for, which is
//! exactly right for a caller and exactly wrong for this test: the newest instruction
//! set is by definition the one nobody has minted yet, so it would be the one kernel
//! never compared against anything. A freshly written `vpshufb` sweep that dropped
//! every second slice would sail through
//! `soundness.rs::every_accelerated_kernel_agrees_with_the_scalar_reference` because
//! that test would still be running `pshufb`.

use sheng::price::Residency;
use sheng::shuffle::Kernel;
use sheng::{Policy, Sieve, Skip};

/// The shapes that harvest a quotient *and* elect skip lanes — the same slate
/// `soundness.rs` uses, since a kernel bug does not care which pattern found it but a
/// tail bug needs long resident runs to be reachable at all.
const PATTERNS: &[&str] = &[
    r"(?-u)WalletService",
    r"(?-u)foo[^\n]*bar",
    r"(?-u)a[^\n]*b",
    r"(?-u)<[^>]*>",
    r"(?-u)(alpha|beta|gamma)",
    r"(?-u)[0-9]{3}-[0-9]{4}",
    r"(?-u)[A-Z][a-z]+Service",
    r"(?-u)#[0-9a-fA-F]{6}",
    r"(?-u)panic!\(",
];

/// xorshift64*, so a failure is reproducible from its seed alone.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// Lengths that straddle every vector width's step and chunk boundaries, plus the
/// remainders either side of each — a 512-bit classifier leaves a different tail than a
/// 256-bit one and a different one again from a 128-bit one, and a dropped remainder is
/// silent rather than loud.
///
/// The widest kernel slices a chunk sixteen ways, so its interesting lengths are the
/// multiples of sixteen either side of a whole step (64) and of a whole chunk (256) —
/// below sixteen bytes it cannot slice at all and the scalar finish covers everything.
fn lengths() -> impl Iterator<Item = usize> {
    (0..=40).chain([
        47, 48, 49, 63, 64, 65, 79, 80, 81, 127, 128, 129, 191, 192, 193, 255, 256, 257, 258, 271,
        272, 273, 511, 512, 513, 1023, 1024,
    ])
}

/// A haystack whose runs are long enough that a skip loop actually jumps and actually
/// lands in its remainder. Uniformly random bytes leave the start block within a byte
/// or two, which never reaches the tail where a vector search goes wrong.
fn resident(rng: &mut Rng, len: usize) -> Vec<u8> {
    (0..len)
        .map(|_| {
            if rng.below(48) == 0 {
                b"<>{}\";-#\n!("[rng.below(11)]
            } else {
                b'a' + rng.below(3) as u8
            }
        })
        .collect()
}

/// A [`Skip`] over exactly `escape`, built the way `Skip::of` reads one out of a real
/// quotient: row `b` sends block 0 away precisely when `b` escapes.
fn skip_over(escape: &[u8]) -> Option<Skip> {
    let mut rows = [[0u8; 16]; 256];
    for &b in escape {
        rows[usize::from(b)][0] = 1;
    }
    Skip::of(&rows, 0)
}

/// Escape sets spanning both instruments — `memchr` at one to three bytes, the nibble
/// classifier above it — and, at the wide end, the widest ASCII set the tables
/// represent.
fn escape_sets() -> Vec<Vec<u8>> {
    vec![
        vec![b'W'],
        vec![b'a', b'b'],
        vec![b'a', b'b', b'g'],
        (b'0'..=b'9').collect(),
        (b'A'..=b'Z').collect(),
        (b'0'..=b'9')
            .chain(b'a'..=b'f')
            .chain(b'A'..=b'F')
            .collect(),
        vec![0x00, 0x01, 0x7F, b'\n'],
        (0..0x80u8).collect(),
    ]
}

/// The classifier, differentiated per kernel rather than through a lane the economics
/// may or may not have elected.
///
/// Driving [`Skip`] directly is what makes this reachable at all: a kernel nobody has
/// minted resolves to an unmeasured calibration, no lane elects a skip under one, and
/// the wide classifier of the newest instruction set would therefore be the one piece
/// of `unsafe` in the crate that no test ever ran. An escape byte is planted at every
/// offset of every length, because a vector search that dropped its remainder does not
/// crash — it reports `None`, and the sieve walks past a real escape into a wrong
/// refutation.
fn classifier_agrees(kernel: Kernel) -> usize {
    let mut checked = 0usize;
    for escape in escape_sets() {
        let Some(skip) = skip_over(&escape) else {
            continue; // refused, which is always a sound answer
        };
        let needle = escape[0];
        let Some(filler) = (0..=255u8).find(|b| !escape.contains(b)) else {
            continue;
        };
        // Past two 512-bit steps, so a whole-block loop and its tail are both live on the
        // widest classifier as well as the narrow ones.
        for len in 1..=144usize {
            for at in 0..len {
                let mut hay = vec![filler; len];
                hay[at] = needle;
                assert_eq!(
                    skip.find(&hay),
                    Some(at),
                    "{kernel:?} missed a planted escape: set={escape:?} len={len} at={at}"
                );
                assert_eq!(
                    skip.find(&hay),
                    skip.find_scalar(&hay),
                    "{kernel:?} disagrees with the definition: set={escape:?} len={len} at={at}"
                );
                checked += 1;
            }
        }
        for b in 0..=255u8 {
            assert_eq!(
                skip.find(&[b]) == Some(0),
                escape.contains(&b),
                "{kernel:?} misclassified {b:#04x} for set={escape:?}"
            );
        }
    }
    checked
}

#[test]
fn every_kernel_this_silicon_can_run_agrees_with_the_scalar_reference() {
    let available = sheng::shuffle::available();
    println!("available kernels: {available:?}");
    assert_eq!(
        available.last(),
        Some(&Kernel::Scalar),
        "every target can run the reference path, so it must terminate the ladder"
    );
    if cfg!(any(target_arch = "aarch64", target_arch = "x86_64")) {
        assert!(
            available.iter().any(|k| k.is_vector()),
            "{}/{} has a byte shuffle but the probe found none — this test would \
             compare the scalar path against itself",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }

    // A kernel the probe did not admit must be unforceable, because that refusal is
    // the entire reason every `unsafe` dispatch arm may trust `kernel()`. Swept from
    // `Kernel::ALL` rather than a list written out here, so a variant added to the enum
    // cannot quietly skip this check by nobody remembering to name it twice.
    for &absent in Kernel::ALL {
        if !available.contains(&absent) {
            assert!(
                !sheng::shuffle::force(absent),
                "{absent:?} is not executable here but force() accepted it"
            );
        }
    }

    let mut swept = 0usize;
    for &kernel in available {
        assert!(
            sheng::shuffle::force(kernel),
            "{kernel:?} is available but force() refused it"
        );
        assert_eq!(sheng::shuffle::kernel(), kernel, "force() did not take");

        // Rebuilt under the forced kernel rather than hoisted, because planning reads
        // the calibration and the calibration is keyed on the kernel — a slate built
        // as one kernel and run as another would be measuring the wrong lane mix.
        let policy = Policy {
            gate: sheng::Gate::Ungated,
            ..Policy::new(Residency::Memory)
        };
        let slate: Vec<(&str, Sieve)> = PATTERNS
            .iter()
            .filter_map(|&p| Sieve::with(p, &policy).ok().map(|s| (p, s)))
            .collect();
        assert!(
            slate.len() * 2 >= PATTERNS.len(),
            "only {} of {} patterns harvested under {kernel:?}",
            slate.len(),
            PATTERNS.len()
        );
        let probed = classifier_agrees(kernel);
        println!(
            "{kernel:?}: {} sieves, {} skip lanes, {probed} classifier probes",
            slate.len(),
            slate.iter().map(|(_, s)| s.skipping()).sum::<usize>()
        );

        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        for (pattern, sieve) in &slate {
            for len in lengths() {
                for round in 0..12 {
                    let hay = if round % 2 == 0 {
                        (0..len).map(|_| (rng.next() & 0xFF) as u8).collect()
                    } else {
                        resident(&mut rng, len)
                    };
                    assert_eq!(
                        sieve.refutes(&hay),
                        sieve.refutes_scalar(&hay),
                        "{kernel:?} disagrees with the reference on {}/{} for \
                         {pattern:?}, {len} bytes (round {round})",
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    );
                    swept += 1;
                }
            }
        }
    }
    println!("{swept} haystacks across {} kernels", available.len());
}
