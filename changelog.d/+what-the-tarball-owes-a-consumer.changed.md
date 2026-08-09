The published `.crate` now carries the crate and nothing that merely operates the
repository that produces it. `.github/` alone was roughly two fifths of the tarball -
nine workflows, a triage script, and a label registry - joined by the git hooks, the
release-please and towncrier configuration, and the unreleased `changelog.d/`
fragments, none of which a consumer holding only the tarball can run or act on. What
ships is now a declared whitelist rather than a subtraction from whatever the working
tree happens to hold: sources, the tests and examples that argue they are right, the
license trail, and `clippy.toml`/`rustfmt.toml`/`deny.toml` so a downstream rebuild's
own checks agree with this repository's. That is half the file count and roughly
thirty percent less on the wire.

`rust-toolchain.toml` is deliberately among the departures: it pins 1.96.0 so this
repository's contributors format and lint identically, which is no business of someone
rebuilding the extracted source, and it would have quietly overridden the 1.95
`rust-version` the sources actually ask for.

Two smaller omissions closed alongside it. `no-std` joins the crate's crates.io
categories - `--no-default-features` is the build the feature table was designed
around, so the audience browsing that category is the one this crate was shaped for -
and a `[package.metadata.docs.rs]` block now builds the documentation with every
feature on, for both x86_64 and aarch64 because the crate's risk surface is two
hand-written kernels and NEON is half of it. It passes `--cfg docsrs`, which turns on
a `doc_cfg` leg in `src/lib.rs` so the pattern constructors and the `Dfa` trait are
labeled with the `regex-automata` feature that carries them instead of rendering as
though they were unconditional.
