# `oss-fuzz/` — the build recipe, kept where the code is

Three files OSS-Fuzz needs and does not read from here. OSS-Fuzz builds from its
own tree, so integrating means copying this directory to `projects/sheng/` in a
fork of [google/oss-fuzz](https://github.com/google/oss-fuzz) and opening a pull
request there.

They live in this repository anyway, because a `build.sh` that exists only in
somebody else's monorepo is a build nobody here can fix. Rename a target in
`fuzz/Cargo.toml` and the thing that breaks is two repositories away, discovered
whenever someone next reads a build log — where `cargo fuzz list` in the version
beside the manifest simply keeps working.

| file | what it is |
| --------------- | ----------------------------------------------------------------- |
| `project.yaml` | language, contact, sanitizer, architecture |
| `Dockerfile` | clone the repository into the Rust base builder |
| `build.sh` | `cargo fuzz build -O`, seed each corpus, copy binaries to `$OUT` |

## Why OSS-Fuzz and not only the campaign in `fuzz.yml`

The campaign here is monthly and bounded; OSS-Fuzz is continuous, and the
difference matters more for this crate than for most. Every target asserts one
property — a refutation is never wrong — and a refutation that *is* wrong
produces no crash, no panic and no wrong-looking output. It silently drops a
match. So the search budget is the whole of the assurance, and there is no
smaller signal to catch it earlier.

The second thing OSS-Fuzz buys is silicon. `shuffle::available()` is what the
`kernels` target sweeps, and on arm64 it holds exactly one entry, because NEON is
baseline there and dispatch is never choosing. x86_64 holds three or four —
`Ssse3` and `Scalar` always, `Avx2` and `Avx512` where the machine has them — and
those wide two are the newest and least-exercised code in the crate, precisely
because dispatch does not elect either one. OSS-Fuzz runs x86_64, which is why
`project.yaml` asks for it specifically rather than by default.

## Before submitting

OSS-Fuzz accepts projects with a level of adoption it judges case by case, so the
recipe being ready is necessary and not sufficient. What is worth confirming
first, in order:

```bash
# 1. The recipe builds and the targets run, in OSS-Fuzz's own image.
git clone --depth 1 https://github.com/google/oss-fuzz
cp -r oss-fuzz/. oss-fuzz/projects/sheng/
cd oss-fuzz
python3 infra/helper.py build_image sheng
python3 infra/helper.py build_fuzzers --sanitizer address sheng
python3 infra/helper.py check_build sheng

# 2. Every target actually executes there, briefly.
python3 infra/helper.py run_fuzzer sheng kernels -- -max_total_time=60
```

`check_build` is the step that catches the failure this recipe is most exposed
to: a target that builds and then rejects every input, which looks identical to a
target finding nothing. `fuzz/fuzz_targets/skip.rs` was written wrong in exactly
that way once — an eighteen-byte header is fine and a four-kilobyte one silently
exceeded libFuzzer's default `-max_len`, so the body never ran.

Reports go to the contact in `project.yaml`. There is deliberately no embargo
argument to make: a soundness bug in a prefilter is a wrong answer rather than a
memory-safety hole, so there is nothing for an attacker to weaponize and no
reason for a fix to wait on disclosure timing.
