//! The skip classifier, held to the transition table it claims to summarize.
//!
//! This is the stage `src/README.md` singles out: the one place where being fast and
//! being wrong look identical from the outside, because a skip that steps over a real
//! escape byte silently loses a match and reports nothing at all.
//!
//! # Why the oracle is the rows and not `find_scalar`
//!
//! `Skip::find` against `Skip::find_scalar` is a differential test of the *searcher*,
//! and both searchers read the same two nibble tables `Skip::of` built. So the pair
//! agrees exactly when the encoding is wrong. The assertion here instead reads the
//! answer straight off the transition rows the skip was built from — the first byte
//! whose row moves the run out of its block — and holds both searchers to that, which
//! puts `Skip::of` under test rather than assuming it.
//!
//! # And why the escape set comes from the fuzzer as a bitmap
//!
//! A harvested quotient's rows are highly structured: the escape sets a slate of
//! patterns produces are a few dozen shapes, all of them the complement of something a
//! human wrote. The hazard lives in the *set*, so the set is what varies here — 128
//! bits, one per ASCII value, which is small enough that a mutation reliably lands
//! inside it and every input reaches a real classifier.
//!
//! That framing also lets the refusal boundary be asserted in both directions, which is
//! the part a differential test structurally cannot see. The boundary is not "ASCII" but
//! "ASCII *once the set is wide enough to need the nibble tables*", and both halves are
//! required:
//!
//! * a set of three values or fewer is `memchr`, which is exact for every byte there is,
//!   so it **must** be accepted however high those bytes are;
//! * a wider set **must** be accepted when every member is ASCII, because `lo[b & 0xF]`
//!   carries one bit per high nibble and ASCII has only eight of them — a decline there
//!   is a skip the crate gave up for nothing;
//! * and a wider set **must** be refused the moment one member reaches `0x80`, because
//!   those eight bits are spent and a ninth would alias onto a member. A `Some` there is
//!   a classifier that steps over real escapes, which is the unsoundness this file is
//!   pointed at.
#![no_main]

use libfuzzer_sys::fuzz_target;
use sheng::Skip;

/// Blocks a quotient can hold, which is what a transition row is indexed by.
const LANES: usize = 16;

/// One bit per ASCII value, which is the whole space the nibble form can encode.
const MAP: usize = 128 / 8;

fuzz_target!(|data: &[u8]| {
    // `block`, then the ASCII escape bitmap, then one byte that may push a non-ASCII
    // value into the set, then the haystack. The header is eighteen bytes so that a
    // mutation late in a corpus entry changes bytes to search without disturbing the
    // set the entry was kept for.
    const HEADER: usize = 1 + MAP + 1;
    if data.len() < HEADER {
        return;
    }
    let (head, hay) = data.split_at(HEADER);
    let block = head[0] % LANES as u8;
    let (map, taint) = (&head[1..1 + MAP], head[1 + MAP]);

    // A byte escapes when its row sends `block` somewhere else, so the rows are the
    // set: escaping bytes point one block over, the rest self-loop. Only column
    // `block` is read, and filling the whole row with one value keeps that explicit.
    let ascii = |b: u8| b < 0x80 && map[usize::from(b) / 8] >> (b % 8) & 1 == 1;
    // The high byte the taint adds, present only when its low bit says so — which is
    // what makes the refusal reachable rather than theoretical.
    let high = (taint & 1 == 1).then_some(0x80 | taint >> 1);
    let leaves = |b: u8| ascii(b) || high == Some(b);

    let elsewhere = (block + 1) % LANES as u8;
    let rows: [[u8; LANES]; 256] =
        core::array::from_fn(|b| [if leaves(b as u8) { elsewhere } else { block }; LANES]);

    let declared: Vec<u8> = (0..=u8::MAX).filter(|&b| leaves(b)).collect();
    let skip = Skip::of(&rows, block);
    // `memchr` takes any three bytes exactly; past that the set has to fit eight
    // high-nibble bits, so one non-ASCII member is a refusal and nothing else is.
    match (declared.len() > 3, high) {
        (true, Some(b)) => {
            assert!(
                skip.is_none(),
                "a {}-value set holding {b:#04x} was encoded into eight high-nibble bits",
                declared.len()
            );
            return;
        },
        _ => assert!(
            skip.is_some(),
            "declined a {}-value set the instrument represents exactly",
            declared.len()
        ),
    }
    let skip = skip.expect("just asserted");
    assert_eq!(
        skip.resident, block,
        "a skip is for the block it was asked for"
    );

    let want = hay.iter().copied().position(leaves);
    assert_eq!(
        skip.find(hay),
        want,
        "classifier disagrees with the rows for block {block}"
    );
    assert_eq!(
        skip.find_scalar(hay),
        want,
        "the scalar reference disagrees with the rows for block {block}"
    );

    // `leaves()` is what prices the skip — `Calibration::rival_per_byte` reads it as
    // the engine's own accelerator set — so a set that is not exactly the escaping
    // bytes misprices every decision the gate makes about this lane, in either
    // direction and without saying so.
    assert_eq!(
        skip.leaves(),
        &declared[..],
        "the priced set is not the escape set"
    );
});
