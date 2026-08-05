`Decline` is exported from the crate root and is the error type of the public
`Projection::of`, but it only derived `Debug` — a caller composing it with `?`
into `Box<dyn std::error::Error>` (the exact shape this crate's own top-level
doc example uses) would not compile, and `BuildError::Shape` had to fall back
to `{d:?}` to print one at all. `Decline` now carries the same `Display` +
`std::error::Error` pair `BuildError` already does, with one prose line per
variant, so both of the crate's public error types answer `?` and `.to_string()`
identically instead of one of them being a `Debug`-only enum wearing an error's
name. `BuildError::Shape`'s message now reads through `Decline`'s own `Display`
rather than its `Debug` form.
