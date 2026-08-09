#!/bin/bash -eu
# Build every `fuzz/fuzz_targets/*.rs` for OSS-Fuzz.
#
# Runs inside the `base-builder-rust` image with `$OUT`, `$SRC` and the sanitizer flags
# already set, so this is only the two steps the image cannot know: build the targets,
# and hand OSS-Fuzz the binaries plus a starting corpus.

cd "$SRC/sheng/fuzz"

# `-O` because the sieve is a SIMD kernel and an unoptimized build does not execute the
# code under test. The kernels are selected at *runtime* from CPUID rather than at
# compile time, so no target-feature flag is needed or wanted here — the `kernels`
# target sweeps whatever the fuzzing host turns out to have, and telling the compiler to
# assume AVX2 would only make an unrelated crash on an older host possible.
cargo fuzz build -O

# `corpus/` is gitignored, so the clone above has none and every target would otherwise
# start from a single empty input. `seeds.sh` cuts seeds from this tree's own source
# text — real bytes, on the same reasoning `examples/common.rs` walks a real corpus
# rather than generating one — which is worth more here than anywhere else: a soundness
# oracle is cheap to evaluate and expensive to *reach*, and a haystack has to be
# thousands of bytes long before it crosses the chunk boundaries the kernel has.
./seeds.sh

# Every target the manifest declares, discovered rather than listed, so adding one to
# `fuzz/Cargo.toml` does not silently leave it unbuilt here.
for target in $(cargo fuzz list); do
    cp "target/x86_64-unknown-linux-gnu/release/$target" "$OUT/"
    zip -jq "$OUT/${target}_seed_corpus.zip" "corpus/$target"/*
done
