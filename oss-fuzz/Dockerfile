# syntax is OSS-Fuzz's, not this repository's: the base image supplies the Rust
# toolchain, `cargo-fuzz`, and the sanitizer flags, so there is nothing to install here.
FROM gcr.io/oss-fuzz-base/base-builder-rust

# `$SRC/sheng` is where `build.sh` expects the checkout, and OSS-Fuzz rebuilds this
# image nightly against the default branch — so the clone is deliberately unpinned. A
# `--depth 1` clone is all a build needs and it is the difference between seconds and
# minutes on a repository with history.
RUN git clone --depth 1 https://github.com/The-Billy-Company/sheng $SRC/sheng

COPY build.sh $SRC/
WORKDIR $SRC/sheng
