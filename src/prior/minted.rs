//! Nothing but measured byte models, one pair of constants per corpus.
//!
//! Same split as [`price::minted`](crate::price): [`super`] holds the model — what a
//! chain *is* and why a memoryless one cannot price a run — and this file holds
//! numbers that came off a corpus, each stamped with the corpus, its commit, and the
//! day it was counted. So re-minting a prior touches one file, and the Markov
//! arithmetic next door can be read with no numbers in view.
//!
//! # Four corpora, because a prior is a claim about bytes rather than about text
//!
//! [`SOURCE`] describes a polyglot code tree, and for a long time it was the only
//! measured prior shipped — which quietly meant that a caller filtering prose, JSON,
//! or logs was being priced under a model of somebody else's Rust. The three corpora
//! beside it are not variations on it; they disagree with it about the coarsest thing
//! a chain can say. Source indentation makes `Space` the most self-following class in
//! the tree; English prose puts exactly one space between words, so the same class
//! is *anti*-persistent there. A model that gets that backwards misprices every
//! `[ ]{2,}` on either corpus.
//!
//! All four are swept together by default — see
//! [`DEFAULT_CHAINS`](super::DEFAULT_CHAINS) — because the gate takes the worst case
//! over the chains it is given, so a set that spans four corpora is strictly harder to
//! satisfy than one that spans a single tree. A caller who knows which corpus they are
//! searching narrows to it in a [`Policy`](crate::Policy) and gets a *looser*, better
//! informed decision; that is the only direction narrowing works in.
//!
//! # Thin rows absorb
//!
//! A row is conditioned on its class occurring, and a class that barely occurs in a
//! corpus gets a row built from a handful of samples: prose holds almost no non-ASCII
//! bytes, and the loghub sample holds none whatsoever. Counted straight, the first
//! prints fractions of a handful and the second prints a row of zeros, which is not a
//! distribution.
//!
//! So a row under the mint's support floor is written **absorbing** — the class always
//! repeats — rather than smoothed toward anything. That is the most persistent row
//! that exists, so it prices every run through the class at the maximum, and it is the
//! same doctrine as [`price::UNMEASURED`](crate::price::UNMEASURED): what was not
//! measured reads as the worst case, never as a guess. `absorbing_rows_are_the_thin_
//! ones` holds the shipped tables to it.

use super::Chain;

/// The measured first-order chain over real source bytes.
///
/// Minted over a polyglot slice of this repository. Re-mint with
/// `cargo run --release --example mint` from the repository root; the
/// `host` / commit stamp travels with the numbers, not this prose.
///
/// The diagonal is the reason this type exists. Digits and non-ASCII bytes
/// repeat far above their marginal share; letters less so. A memoryless prior
/// therefore under-prices a long digit run by many orders of magnitude — which
/// is precisely the error that let a filter rejecting essentially nothing look
/// like one rejecting everything.
///
/// `Space` never reaching `Break` is real rather than a rounding artifact: this tree
/// is linted, so trailing whitespace before a newline is effectively absent.
// A transition matrix is one table, and reading it by row against the header below
// is the whole point — so the row-per-line layout is pinned rather than reflowed.
#[rustfmt::skip]
pub const SOURCE: Chain = Chain {
    //     Space     Break     Lower     Upper     Digit     Punct      High
    next: [
        [0.451677, 0.000000, 0.333127, 0.042422, 0.016962, 0.149964, 0.005848], // Space
        [0.713848, 0.104793, 0.063638, 0.016030, 0.000283, 0.101210, 0.000199], // Break
        [0.082433, 0.008042, 0.768254, 0.030242, 0.009533, 0.101450, 0.000046], // Lower
        [0.042460, 0.007471, 0.514333, 0.356499, 0.002727, 0.076414, 0.000095], // Upper
        [0.110313, 0.037564, 0.075528, 0.023526, 0.386326, 0.365925, 0.000818], // Digit
        [0.211487, 0.139038, 0.299459, 0.076937, 0.020348, 0.252417, 0.000314], // Punct
        [0.066678, 0.008770, 0.001777, 0.000535, 0.001748, 0.003810, 0.916681], // High
    ],
    start: [0.181717, 0.027072, 0.570280, 0.055983, 0.018575, 0.132492, 0.013881],
};

/// Marginal frequency of every byte value over the same minted corpus as [`SOURCE`].
///
/// **Per-byte, and that resolution is load-bearing.** The class chain carries how
/// bytes *cluster*; this carries how often each one occurs, and the two answer
/// different questions. Pricing an engine's escape set at class resolution treats `a`
/// and `f` as equally common when `a` occurs about three times as often — which is
/// exactly the difference between a pattern whose accelerator trips constantly (worth
/// fronting) and one whose accelerator earns its keep (not worth fronting). Arming on
/// the class average did both wrong at once.
///
/// Minted alongside [`SOURCE`] on the same corpus. The evidence that the
/// resolution matters is in the excursion solver's own spread: read at class
/// resolution, inverted lead-byte values disagree by about tenfold; read from
/// this table they collapse into a narrow band. The variance was the
/// approximation, not the measurement.
pub const SOURCE_BYTES: [f64; 256] = [
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.02348194, 0.02707233, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.15823466, 0.00093228, 0.01212123, 0.00084917, 0.00016688, 0.00035293, 0.00058606, 0.00179882,
    0.01068197, 0.01038673, 0.00162193, 0.00150419, 0.01070865, 0.00637019, 0.01385492, 0.00496256,
    0.00523016, 0.00400453, 0.00219353, 0.00122989, 0.00159107, 0.00078858, 0.00147180, 0.00072862,
    0.00097900, 0.00035768, 0.00707436, 0.00110169, 0.00086292, 0.00846172, 0.00127486, 0.00011155,
    0.00020109, 0.00435993, 0.00116575, 0.00333640, 0.00214529, 0.00586268, 0.00200241, 0.00110330,
    0.00083414, 0.00317845, 0.00018502, 0.00039273, 0.00256395, 0.00203484, 0.00377334, 0.00244100,
    0.00207299, 0.00029789, 0.00431163, 0.00501794, 0.00464331, 0.00189854, 0.00103260, 0.00053590,
    0.00033695, 0.00035935, 0.00009711, 0.00275525, 0.00427699, 0.00275255, 0.00001399, 0.01390198,
    0.00442084, 0.03637424, 0.00817733, 0.02109737, 0.02265156, 0.07370121, 0.01314766, 0.01015872,
    0.01218381, 0.04055988, 0.00110193, 0.00451404, 0.02458769, 0.01465999, 0.04106374, 0.03908288,
    0.01906260, 0.00161610, 0.04674633, 0.03927468, 0.05454883, 0.01670062, 0.00695385, 0.00526917,
    0.00823928, 0.00747249, 0.00133413, 0.00376073, 0.00083514, 0.00375906, 0.00002829, 0.00000000,
    0.00421794, 0.00000110, 0.00001260, 0.00000134, 0.00000124, 0.00000061, 0.00015301, 0.00000790,
    0.00001624, 0.00002082, 0.00000115, 0.00000051, 0.00000202, 0.00000034, 0.00000030, 0.00000021,
    0.00012277, 0.00000094, 0.00015467, 0.00000512, 0.00420130, 0.00011869, 0.00000476, 0.00001790,
    0.00000150, 0.00000086, 0.00000103, 0.00000013, 0.00000583, 0.00000068, 0.00000024, 0.00000232,
    0.00000219, 0.00000112, 0.00000693, 0.00000202, 0.00000609, 0.00000750, 0.00002823, 0.00001946,
    0.00000007, 0.00000179, 0.00000098, 0.00000192, 0.00000080, 0.00000022, 0.00000025, 0.00000016,
    0.00000077, 0.00000482, 0.00000504, 0.00000228, 0.00000031, 0.00000454, 0.00000109, 0.00003290,
    0.00000122, 0.00000089, 0.00000067, 0.00000204, 0.00000256, 0.00000174, 0.00000012, 0.00000018,
    0.00000000, 0.00000000, 0.00006262, 0.00002132, 0.00000003, 0.00000016, 0.00000000, 0.00000001,
    0.00000000, 0.00000305, 0.00000147, 0.00000147, 0.00000119, 0.00000000, 0.00001351, 0.00000446,
    0.00000129, 0.00000048, 0.00000001, 0.00000000, 0.00000009, 0.00000000, 0.00000000, 0.00000000,
    0.00000024, 0.00000055, 0.00000000, 0.00000006, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000001, 0.00000106, 0.00454794, 0.00000024, 0.00000058, 0.00000043, 0.00000042, 0.00000010,
    0.00000021, 0.00000003, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000021,
    0.00000083, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
];

/// English literary prose, as a first-order chain.
///
/// Minted over the NLTK `gutenberg` plain-text books. Pinned by commit and
/// re-minted by `.github/workflows/priors.yml`; see it for the exact tree.
///
/// The disagreement with [`SOURCE`] is not a matter of degree. Prose separates words
/// with exactly one space, so `Space` is **anti**-persistent here — where source
/// indentation makes it the most self-following class in the tree. Under the source
/// chain a run of spaces is the likeliest thing in the document; under this one it
/// is nearly impossible.
///
/// `Break` persistence is paragraph breaks; `Digit` persistence is chapter numbers
/// and years, which arrive in short bursts and nowhere else. `High` absorbs because
/// this corpus is essentially 7-bit ASCII — a handful of non-ASCII bytes is not a
/// measurement, so the row reads as the worst case instead of as those samples. A
/// caller searching UTF-8 prose with curly quotes and accents wants their own mint
/// there; the default sweep meanwhile prices it under [`SOURCE`]'s measured `High`
/// row.
#[rustfmt::skip]
pub const PROSE: Chain = Chain {
    //     Space     Break     Lower     Upper     Digit     Punct      High
    next: [
        [0.025885, 0.005468, 0.861940, 0.098256, 0.003497, 0.004953, 0.000000], // Space
        [0.030117, 0.323876, 0.367744, 0.160911, 0.081990, 0.035361, 0.000000], // Break
        [0.187825, 0.012269, 0.757571, 0.000057, 0.000001, 0.042276, 0.000001], // Lower
        [0.123993, 0.006009, 0.722462, 0.125761, 0.000020, 0.021749, 0.000007], // Upper
        [0.292365, 0.008831, 0.000493, 0.000085, 0.400484, 0.297743, 0.000000], // Digit
        [0.552217, 0.190790, 0.078299, 0.039155, 0.068389, 0.071148, 0.000002], // Punct
        [0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 1.000000], // High — absorbing
    ],
    start: [0.169649, 0.025889, 0.731269, 0.025611, 0.008940, 0.038641, 0.000001],
};

/// Marginal frequency of every byte value over the same corpus as [`PROSE`].
///
/// Reads as English rather than as code: letter frequencies shift, and almost
/// nothing sits above `0x7F`. The punctuation is the tell — `{`, `<`, `;` and `=`
/// round to zero here and are among the commonest bytes in a code tree, which is
/// exactly the escape set a rival engine would have been priced on.
pub const PROSE_BYTES: [f64; 256] = [
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.02178293, 0.00000000, 0.00000000, 0.00410656, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000017, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.16964886, 0.00073431, 0.00264217, 0.00000000, 0.00000102, 0.00000025, 0.00000721, 0.00183850,
    0.00015441, 0.00015492, 0.00002400, 0.00000008, 0.01631161, 0.00237677, 0.00694046, 0.00000195,
    0.00047315, 0.00233149, 0.00162982, 0.00105322, 0.00078748, 0.00060806, 0.00055633, 0.00051131,
    0.00050113, 0.00048782, 0.00403830, 0.00236931, 0.00000017, 0.00000008, 0.00000025, 0.00087694,
    0.00000025, 0.00274003, 0.00117558, 0.00079520, 0.00113039, 0.00090941, 0.00069277, 0.00083836,
    0.00126249, 0.00362426, 0.00071727, 0.00017145, 0.00128039, 0.00132126, 0.00060594, 0.00133508,
    0.00056507, 0.00005808, 0.00095775, 0.00142708, 0.00226493, 0.00012728, 0.00010082, 0.00108036,
    0.00001518, 0.00033112, 0.00008378, 0.00001111, 0.00000000, 0.00001111, 0.00000000, 0.00009463,
    0.00001772, 0.05926144, 0.01068249, 0.01496364, 0.03282901, 0.09402714, 0.01704940, 0.01375024,
    0.05391646, 0.04536035, 0.00063485, 0.00548226, 0.03054382, 0.01818403, 0.05154995, 0.05616664,
    0.01098156, 0.00058228, 0.04164282, 0.04579144, 0.06787318, 0.02125865, 0.00700736, 0.01598795,
    0.00076153, 0.01459598, 0.00038471, 0.00000000, 0.00000000, 0.00003298, 0.00000008, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000017, 0.00000000,
    0.00000017, 0.00000017, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000008, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
];

/// Machine-generated JSON, as a first-order chain.
///
/// Minted over `simdjson/simdjson-data` — the corpus that project benchmarks its
/// parsers against, so it spans pretty-printed meshes, minified API dumps, GeoJSON,
/// and Twitter payloads rather than one shape of object. Pinned by commit in
/// `.github/workflows/priors.yml`.
///
/// This is the corpus where **nothing is rare**, and that makes it the hardest of the
/// four for a filter to clear. Every class repeats more often than it occurs, most of
/// them heavily, because structure is what JSON is made of: indentation runs,
/// numeric arrays, key strings, and UTF-8 string bodies each persist.
///
/// `Digit` is the one to read against [`SOURCE`]: a large share of these bytes are
/// digits and most of a digit's successors are digits again, where a code tree puts
/// digits near the noise floor. So `[0-9]{3}-[0-9]{4}` — a phone number, and about as
/// selective as a pattern gets in source — refutes almost nothing in a mesh file.
#[rustfmt::skip]
pub const JSON: Chain = Chain {
    //     Space     Break     Lower     Upper     Digit     Punct      High
    next: [
        [0.698783, 0.015873, 0.153236, 0.028972, 0.037176, 0.063080, 0.002880], // Space
        [0.814780, 0.035390, 0.000035, 0.000006, 0.024023, 0.125731, 0.000035], // Break
        [0.088815, 0.000023, 0.754992, 0.016302, 0.063173, 0.076055, 0.000640], // Lower
        [0.036807, 0.000005, 0.537203, 0.311507, 0.041417, 0.071473, 0.001588], // Upper
        [0.003238, 0.001086, 0.073545, 0.006788, 0.764063, 0.151093, 0.000187], // Digit
        [0.056101, 0.033982, 0.089658, 0.011370, 0.131420, 0.676306, 0.001163], // Punct
        [0.036010, 0.000035, 0.014402, 0.002506, 0.002482, 0.023714, 0.920850], // High
    ],
    start: [0.171384, 0.011198, 0.310596, 0.020803, 0.243059, 0.229849, 0.013110],
};

/// Marginal frequency of every byte value over the same corpus as [`JSON`].
///
/// The digits are the shape of it: far more frequent than in a code tree. `"` is
/// among the commonest non-space bytes, which is what makes a quote a hopeless
/// thing to accelerate on here and a fine one in prose.
pub const JSON_BYTES: [f64; 256] = [
    0.00001002, 0.00000089, 0.00000039, 0.00000004, 0.00000025, 0.00000007, 0.00000007, 0.00000000,
    0.00000000, 0.00000959, 0.01087278, 0.00000018, 0.00000007, 0.00032545, 0.00000057, 0.00000018,
    0.00000067, 0.00000007, 0.00000000, 0.00000000, 0.00000004, 0.00000018, 0.00000007, 0.00000018,
    0.00000060, 0.00000014, 0.00000007, 0.00000011, 0.00000004, 0.00000004, 0.00000000, 0.00000000,
    0.17137454, 0.00001498, 0.03894973, 0.00006578, 0.00007512, 0.00010741, 0.00004747, 0.00011438,
    0.00057890, 0.00045679, 0.00014356, 0.00036776, 0.03169389, 0.00596941, 0.01479385, 0.00347982,
    0.04611800, 0.02458733, 0.02054139, 0.01949115, 0.01848541, 0.01844920, 0.01819625, 0.02009660,
    0.01831194, 0.03878189, 0.01336814, 0.00006086, 0.00006241, 0.00013357, 0.00005385, 0.00011251,
    0.00024739, 0.00133342, 0.00103088, 0.00163264, 0.00086307, 0.00097292, 0.00067586, 0.00094287,
    0.00044745, 0.00166882, 0.00084285, 0.00028332, 0.00055121, 0.00084466, 0.00069455, 0.00078699,
    0.00130556, 0.00015804, 0.00062655, 0.00156343, 0.00119273, 0.00100801, 0.00045499, 0.00035954,
    0.00015658, 0.00020615, 0.00019999, 0.09407188, 0.01032044, 0.00675487, 0.00000457, 0.00164903,
    0.00001296, 0.02800997, 0.00884420, 0.01592063, 0.01720658, 0.03628374, 0.01042331, 0.00602934,
    0.00780731, 0.02240096, 0.00128506, 0.00179514, 0.01370439, 0.00845071, 0.01988943, 0.02130937,
    0.00861377, 0.00041449, 0.01888209, 0.01916655, 0.02297908, 0.01048219, 0.00286885, 0.00274356,
    0.00082696, 0.00370494, 0.00055387, 0.00381731, 0.00034089, 0.00196804, 0.00001136, 0.00003179,
    0.00042706, 0.00075523, 0.00036992, 0.00016267, 0.00015028, 0.00007438, 0.00009452, 0.00007604,
    0.00009024, 0.00007240, 0.00008305, 0.00010522, 0.00011272, 0.00005813, 0.00003622, 0.00006748,
    0.00007031, 0.00005275, 0.00006556, 0.00009357, 0.00007332, 0.00007548, 0.00004142, 0.00009374,
    0.00008872, 0.00008132, 0.00005661, 0.00007364, 0.00011509, 0.00009983, 0.00004238, 0.00008539,
    0.00009045, 0.00008118, 0.00003969, 0.00004316, 0.00007293, 0.00004365, 0.00006418, 0.00011031,
    0.00010723, 0.00007976, 0.00006804, 0.00006206, 0.00004804, 0.00007339, 0.00009789, 0.00006815,
    0.00027245, 0.00008337, 0.00020951, 0.00010964, 0.00012178, 0.00022849, 0.00003501, 0.00005211,
    0.00030389, 0.00010550, 0.00016306, 0.00015864, 0.00014076, 0.00022332, 0.00033809, 0.00005282,
    0.00000004, 0.00000000, 0.00004149, 0.00012840, 0.00001296, 0.00001586, 0.00000018, 0.00000011,
    0.00000004, 0.00000004, 0.00000004, 0.00000078, 0.00000219, 0.00000000, 0.00002050, 0.00000503,
    0.00207099, 0.00050742, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00013764, 0.00008104, 0.00000641, 0.00002170, 0.00000000, 0.00000011, 0.00000000, 0.00000000,
    0.00000120, 0.00000046, 0.00013874, 0.00097030, 0.00009707, 0.00026622, 0.00018044, 0.00013435,
    0.00008861, 0.00005937, 0.00004429, 0.00009081, 0.00016373, 0.00004521, 0.00000032, 0.00002857,
    0.00000057, 0.00000000, 0.00000641, 0.00000007, 0.00000007, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000004, 0.00000000, 0.00000007, 0.00000000, 0.00000004, 0.00000014,
];

/// Production service logs, as a first-order chain.
///
/// Minted over the `logpai/loghub` system samples — many real emitters rather than
/// many samples of one, which is the only way a log prior is about logs and not
/// about one team's formatter. Pinned by commit in `.github/workflows/priors.yml`.
///
/// `Punct` is the one that is genuinely backwards from every other corpus here: a log
/// line is fields separated by single delimiters, so punctuation alternates with
/// content rather than clustering, and it is *anti*-persistent where source text and
/// JSON both cluster. Meanwhile a timestamp puts a large share of every line in
/// `Digit` at high persistence — so `[0-9]{4}` refutes essentially no log line and
/// `[;:]{2}` refutes nearly all of them, which is the reverse of the source ordering.
///
/// Two honest caveats about the corpus rather than about logs. `Break` persistence
/// is inflated by CRLF line endings in most samples — errs pessimistic, since a
/// chain that over-states persistence over-states fallthrough and can only decline —
/// but a caller whose logs are LF should expect far less. `High` absorbs because
/// these samples hold no non-ASCII byte at all, on the same terms as [`PROSE`].
#[rustfmt::skip]
pub const LOG: Chain = Chain {
    //     Space     Break     Lower     Upper     Digit     Punct      High
    next: [
        [0.115145, 0.003883, 0.389784, 0.162794, 0.229300, 0.099093, 0.000000], // Space
        [0.000000, 0.483896, 0.032257, 0.099077, 0.258052, 0.126719, 0.000000], // Break
        [0.073103, 0.004604, 0.768703, 0.026720, 0.035110, 0.091760, 0.000000], // Lower
        [0.088741, 0.004730, 0.384497, 0.404898, 0.067705, 0.049428, 0.000000], // Upper
        [0.103820, 0.011640, 0.045237, 0.000987, 0.633708, 0.204608, 0.000000], // Digit
        [0.178150, 0.011247, 0.187119, 0.102203, 0.462466, 0.058814, 0.000000], // Punct
        [0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 0.000000, 1.000000], // High — absorbing
    ],
    start: [0.097946, 0.013899, 0.429553, 0.068936, 0.272700, 0.116966, 0.000000],
};

/// Marginal frequency of every byte value over the same corpus as [`LOG`].
///
/// Digits dominate via timestamps and process ids; `:` and `,` are the delimiters
/// that make [`LOG`]'s `Punct` row anti-persistent. Every byte above `0x7F` is zero,
/// which is what [`LOG`]'s absorbing `High` row states in the other direction.
pub const LOG_BYTES: [f64; 256] = [
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00717378, 0.00000000, 0.00000000, 0.00672546, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.09794601, 0.00003252, 0.00057055, 0.00120636, 0.00056337, 0.00000628, 0.00000606, 0.00015430,
    0.00211242, 0.00210838, 0.00108054, 0.00002288, 0.00460722, 0.02243638, 0.02518034, 0.00628432,
    0.05388960, 0.05133223, 0.03341560, 0.02281517, 0.02122508, 0.02319666, 0.01789061, 0.01701034,
    0.01594887, 0.01597555, 0.02594040, 0.00039674, 0.00015430, 0.00430849, 0.00012582, 0.00000673,
    0.00093387, 0.00360562, 0.00240913, 0.00462067, 0.00298371, 0.00340220, 0.00368725, 0.00058288,
    0.00088812, 0.00483889, 0.00199669, 0.00120613, 0.00262936, 0.00329209, 0.00633972, 0.00320821,
    0.00240150, 0.00047254, 0.00528250, 0.00673197, 0.00336049, 0.00171119, 0.00065443, 0.00197180,
    0.00012739, 0.00004194, 0.00048959, 0.00534193, 0.00003319, 0.00533969, 0.00000000, 0.00603067,
    0.00000000, 0.03521135, 0.01006891, 0.02429537, 0.02341622, 0.05288419, 0.00807603, 0.00844630,
    0.00925884, 0.02520255, 0.00051986, 0.00673982, 0.01594303, 0.01236254, 0.02846682, 0.03358627,
    0.01217460, 0.00081299, 0.03106411, 0.02412671, 0.03358134, 0.01373059, 0.00622556, 0.00334412,
    0.00422797, 0.00521163, 0.00057481, 0.00005203, 0.00137815, 0.00005203, 0.00050057, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
    0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000, 0.00000000,
];

#[cfg(test)]
mod tests {
    use super::super::{CLASSES, Class};
    use super::*;

    /// A byte table is a histogram over the corpus it was counted on, so it has to
    /// sum to one — and the class marginals beside it have to be the *same* count
    /// regrouped, or the two halves of a prior describe different corpora. Six
    /// decimal places is what the mint prints, so that is the tolerance.
    #[test]
    fn each_byte_table_is_the_same_corpus_its_chain_is() {
        for (name, chain, freq) in [
            ("SOURCE", SOURCE, SOURCE_BYTES),
            ("PROSE", PROSE, PROSE_BYTES),
            ("JSON", JSON, JSON_BYTES),
            ("LOG", LOG, LOG_BYTES),
        ] {
            let total: f64 = freq.iter().sum();
            assert!((total - 1.0).abs() < 1e-4, "{name}_BYTES sums to {total}");
            let mut grouped = [0.0f64; CLASSES];
            for (b, &p) in (0..=u8::MAX).zip(&freq) {
                grouped[Class::of(b) as usize] += p;
            }
            for (class, (&byte_wise, &declared)) in
                Class::ALL.into_iter().zip(grouped.iter().zip(&chain.start))
            {
                assert!(
                    (byte_wise - declared).abs() < 1e-4,
                    "{name} {class:?}: bytes say {byte_wise}, start says {declared}"
                );
            }
        }
    }

    /// The absorbing rows are exactly the ones the corpus could not speak for, and a
    /// row that absorbs has to be *unreachable enough* that absorbing costs nothing
    /// real — otherwise the pessimism is not a floor, it is the model.
    ///
    /// So this pins both halves of that claim: a row is absorbing only where the class
    /// is essentially absent, and every other row is a genuine measurement with
    /// off-diagonal mass in it. Ship a thin row without the floor and the first half
    /// fails; smooth one toward the marginal instead and the second does.
    #[test]
    fn absorbing_rows_are_the_thin_ones() {
        for (name, chain) in [
            ("SOURCE", SOURCE),
            ("PROSE", PROSE),
            ("JSON", JSON),
            ("LOG", LOG),
        ] {
            for (i, class) in Class::ALL.into_iter().enumerate() {
                let absorbing = chain.next[i][i] == 1.0;
                assert_eq!(
                    absorbing,
                    chain.start[i] < 1e-5,
                    "{name} {class:?}: absorbing={absorbing} at marginal {}",
                    chain.start[i]
                );
            }
        }
    }
}
