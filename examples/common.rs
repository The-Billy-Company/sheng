//! What both examples need from the outside world: real bytes to measure, and an
//! honest label for the machine that measured them.
//!
//! Kept in one place because the alternative was two copies of a corpus walker rooted
//! at `"."` — which quietly meant "only correct when run from a repository root", the
//! kind of tie that makes a measurement look portable and not be.

// Each example compiles this module in full, so whatever the *other* one uses reads as
// dead here. The alternative — one walker per example — is the duplication this file
// exists to remove.
#![allow(dead_code)]

/// The tree to read real source text from.
///
/// `$SHENG_CORPUS` wins, because the whole point of re-minting is to measure *your*
/// corpus rather than inherit somebody else's. Failing that, the enclosing checkout,
/// found by climbing to a `.git` so it works whether this crate sits inside a monorepo
/// or was extracted and published alone. Failing that, the crate itself — small, but
/// source text by construction and always present.
pub fn root() -> std::path::PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(dir) = std::env::var_os("SHENG_CORPUS") {
        return dir.into();
    }
    here.ancestors()
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(here)
        .to_path_buf()
}

/// Which file extensions [`root`] is read for.
///
/// `$SHENG_KINDS` — comma-separated, dots optional — is the other half of pointing a
/// mint at a corpus, and without it `$SHENG_CORPUS` cannot reach one: a tree of prose,
/// JSON, or logs is invisible to the source-tree default no matter where the root is
/// aimed. That is why the shipped priors all described a code tree.
fn kinds() -> Vec<String> {
    const SOURCE: &str = "rs,zig,go,py,ts,tsx,md,toml,sql,swift";
    std::env::var("SHENG_KINDS")
        .unwrap_or_else(|_| SOURCE.into())
        .split(',')
        .map(|kind| kind.trim().trim_start_matches('.').to_owned())
        .filter(|kind| !kind.is_empty())
        .collect()
}

/// Up to `files` non-empty source files from [`root`].
pub fn corpus_files(files: usize) -> Vec<Vec<u8>> {
    walk(files, usize::MAX)
        .into_iter()
        .map(|(_, bytes)| bytes)
        .collect()
}

/// Enough files from [`root`] to reach `bytes` of source text.
///
/// A byte budget rather than a file count for anything statistical: rare byte classes
/// are counted per byte, so what a prior needs is a volume of text, and file sizes here
/// span four orders of magnitude.
pub fn corpus_bytes(bytes: usize) -> Vec<Vec<u8>> {
    walk(usize::MAX, bytes)
        .into_iter()
        .map(|(_, bytes)| bytes)
        .collect()
}

/// Up to `files` non-empty source files from [`root`], each beside the path it was
/// read from — for a caller that needs to name which file a disagreement came from.
pub fn corpus_paths(files: usize) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    walk(files, usize::MAX)
}

/// Real bytes only, stopping at whichever budget binds first. A synthetic fill would
/// answer every question this crate asks with whatever generator wrote it, which is
/// the one thing a prior must not do.
fn walk(files: usize, bytes: usize) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let kinds = kinds();
    let mut out: Vec<(std::path::PathBuf, Vec<u8>)> = Vec::new();
    let mut held = 0usize;
    let mut stack = vec![root()];
    let full =
        |out: &Vec<(std::path::PathBuf, Vec<u8>)>, held: usize| out.len() >= files || held >= bytes;
    while let Some(dir) = stack.pop() {
        if full(&out, held) {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // Dotfiles, build output and vendored trees are not the corpus anyone
            // greps, and `target/` alone would swamp the sample with our own artifacts.
            if name.starts_with('.') || matches!(&*name, "target" | "node_modules" | "vendor") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !kinds.iter().any(|kind| kind.eq_ignore_ascii_case(ext)) {
                continue;
            }
            if let Ok(text) = std::fs::read(&path)
                && !text.is_empty()
            {
                held += text.len();
                out.push((path, text));
            }
            if full(&out, held) {
                break;
            }
        }
    }
    out
}

/// The silicon a measurement came from, from `std` alone.
///
/// No `uname` subprocess: a mint has to be runnable on any target that can run the
/// crate, and a shell-out is both unportable and unnecessary when the compiler already
/// knows the answer.
///
/// Kernel-free, deliberately, because one machine now yields one row per kernel: a
/// banner that named a kernel would be labeling several measurements with whichever one
/// happened to be resolved when it printed.
pub fn machine() -> String {
    let cores = std::thread::available_parallelism().map_or(0, std::num::NonZero::get);
    format!(
        "{} {} · {cores} logical cores",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// [`machine`] plus the kernel currently resolved — what a single [`Calibration`] row's
/// `host` field records.
///
/// The kernel belongs here because it is the field that decides whether a calibration
/// applies at all: a `pshufb` number describes no machine that lacks `pshufb`, and a
/// `vpshufb` one describes no machine that lacks `vpshufb` either.
///
/// [`Calibration`]: sheng::price::Calibration
pub fn host() -> String {
    format!("{} · {:?} kernel", machine(), sheng::shuffle::kernel())
}

/// Today, as `YYYY-MM-DD`, without a `date` subprocess.
///
/// Days-to-civil is Howard Hinnant's `civil_from_days`
/// (<https://howardhinnant.github.io/date_algorithms.html#civil_from_days>), which is
/// exact for the whole proleptic Gregorian range and shorter than the shell-out it
/// replaces.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    #[allow(clippy::cast_possible_wrap)]
    let days = (secs / 86_400) as i64;
    let z = days + 719_468; // shift the epoch to 0000-03-01, so leap day lands last
    let era = z.div_euclid(146_097); // 400-year cycle
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153; // March-based month, [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
