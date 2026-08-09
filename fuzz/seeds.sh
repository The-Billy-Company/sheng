#!/bin/bash -eu
# Top up each target's corpus with seeds cut from this repository's own source text.
#
# `corpus/` is machine-local and gitignored, so a fresh clone — a CI runner, an OSS-Fuzz
# builder — starts every target cold. That costs real budget: the first thousands of
# executions go on rediscovering that a haystack wants to be text and wants to be long
# enough to cross `shuffle::CHUNK`, which are facts about the input shape rather than
# anything a search should have to find.
#
# So the seeds are cut from bytes that are already here, on the same reasoning
# `examples/common.rs` walks a real tree rather than generating one: a synthetic haystack
# answers every question with whatever wrote it. Idempotent, and it only ever adds — a
# corpus restored from cache is left alone and merely joined.
#
#     ./seeds.sh                # every target
#     ./seeds.sh soundness      # one
#
# Deliberately not called from `cargo fuzz` itself. A corpus is an input to a search and
# the search should be startable without it.

cd "$(dirname "$0")"
targets=${*:-"soundness kernels skip"}

# How many patterns the two pattern-driven targets hold. Their first input byte selects
# one, and a seed that only ever named pattern 0 would leave fourteen automata to be
# found by mutation.
patterns=$(grep -c '^    r' fuzz_targets/soundness.rs)

# Real text, largest first, so a seed is long enough to reach the chunked kernel rather
# than only its scalar tail. 8 KiB is thirty-two chunks, past every boundary the
# composition kernel has.
sources=$(find ../src ../examples -name '*.rs' | sort)

for target in $targets; do
    mkdir -p "corpus/$target"
    n=0
    for file in $sources; do
        # One seed per (pattern, file) pair, capped so a big tree does not bury the
        # corpus in near-duplicates of itself.
        index=$((n % patterns))
        case $target in
            # `[pattern index] ++ haystack`.
            soundness | kernels) header=$(printf '\\x%02x' $index) ;;
            # `[block] ++ [16-byte ASCII escape bitmap] ++ [taint] ++ haystack`. The map
            # is `0x0F` repeated: low nibbles 0..3 paired with high nibbles 0..3, which
            # is a wide set the classifier represents exactly and no source file
            # resembles. A cold `skip` corpus otherwise starts from the empty set, where
            # every answer is `None` and nothing is classified at all. The block varies
            # with the seed for the same reason the pattern index does above — otherwise
            # every seed is the same input with different trailing bytes.
            skip) header="$(printf '\\x%02x' $index)\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x0f\x00" ;;
            *)
                echo "unknown target: $target" >&2
                exit 1
                ;;
        esac
        # Named for what it is rather than hashed, so re-running overwrites the same
        # seeds instead of leaving a fresh set beside the last one.
        # `%b` rather than a bare format string: the escapes belong to the *argument*, so
        # a header is data being decoded and not a format being trusted.
        {
            printf %b "$header"
            head -c 8192 "$file"
        } >"corpus/$target/seed-$index-$(basename "$file" .rs)"
        n=$((n + 1))
    done
    echo "$target: $(find "corpus/$target" -type f | wc -l | tr -d ' ') inputs after seeding"
done
