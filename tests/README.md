# `sheng/tests/`

**A false reject is a missed match.** That is the only failure this crate can commit
that no downstream check would ever catch, so the suite here is built around it rather
than around coverage.

Two files. `soundness.rs` asks whether a sieve ever lies about a document; `policy.rs`
asks whether the decision to use one is secretly a claim about a single laptop.

`soundness.rs` holds four properties:

| test                                                 | what it would catch                                                                                                                                      |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `a_refutation_is_never_wrong_on_mutated_haystacks`   | a quotient that rejects a matching document, over 4,000 haystacks spliced from the pattern's own alphabet so real matches actually occur                 |
| `a_refutation_is_never_wrong_on_real_source_bytes`   | the same, against this repository's real files rather than a generator                                                                                   |
| `every_accelerated_kernel_agrees_with_the_scalar_reference` | a vector lane diverging from the scalar definition — and, first, dispatch quietly having chosen `scalar`, which would make the comparison vacuous |
| `an_armed_sieve_retires_most_of_what_it_sees`        | a sieve that armed on a model's promise and then rejected almost nothing                                                                                 |

## Why the first three run ungated

Soundness is a property of the **construction**, not of the economics, so those three
test every pattern that harvests a quotient via `Sieve::ungated` — not the minority the
cost gate admits. Testing only the armed ones would silently shrink this suite every
time the gate got stricter, which is exactly backwards: the risk of a false reject
exists the moment a quotient exists, whether or not anyone would profit from running it.

`an_armed_sieve_retires_most_of_what_it_sees` is the one that deliberately uses the
gated path, because it is auditing the gate's claim rather than the kernel's.

## What `policy.rs` is for

A sieve that is sound everywhere can still be a bad bargain on silicon nobody measured,
and that failure is silent: the filter works, it just costs more than it saves. So these
six pin the economics as a **replaceable policy** rather than a global fact.

| test                                                     | what it would catch                                                                      |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `an_unmeasured_machine_declines_instead_of_guessing`     | an unknown target inheriting another machine's ratios and shipping a slowdown            |
| `waiving_the_gate_still_builds_on_an_unmeasured_machine` | the refusal hardening from a policy into an incapacity                                   |
| `a_caller_can_price_a_machine_the_crate_never_measured`  | a `Calibration` argument that is accepted and then ignored                               |
| `the_prior_reaches_the_decision`                         | chains that decorate the API without reaching the model, or a sweep that is not monotone |
| `longer_documents_are_harder_to_justify`                 | the nominal length dropping out of the amortization                                      |
| `every_shipped_row_names_a_real_machine`                 | a coefficient with no provenance, or a scalar row pricing vector economics               |

The end-to-end economic check is not here — it needs a large corpus and several
seconds, so it lives in `examples/survey.rs`, which asserts.
