# `sheng/tests/`

**A false reject is a missed match.** That is the only failure this crate can commit
that no downstream check would ever catch, so the suite here is built around it rather
than around coverage.

`soundness.rs` asks whether a sieve ever lies about a document; `policy.rs` asks whether
the decision to use one is secretly a claim about a single laptop; `rival.rs` asks what
the decision is even a decision *about*.

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

## What `rival.rs` is for

`policy.rs` pins the economics against one rival: `regex-automata`. But an economic
decline is a verdict against *a particular* rival, and that one is among the fastest byte
scanners in existence — so "this filter is not good enough" and "this filter is competing
with something very hard to beat" are different findings the gate reports identically.
`Policy::rival` is the seam that separates them, and these three keep it honest in both
directions.

| test                                                        | what it would catch                                                                          |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `a_costly_confirm_arms_a_sieve_the_engine_declined`         | a stated rival price that is accepted and then ignored, or a gate not monotone in it         |
| `no_rival_price_rescues_a_filter_that_retires_nothing`      | an expensive rival overriding the gate rather than pricing it — arming a filter with no selectivity |
| `a_nonsense_rival_price_declines_rather_than_arming_or_panicking` | a price that is not a price reaching the comparison; a negative one **inverts** the inequality and arms |

The second is what makes the first safe to offer, and the third is why `CostFact::pays`
guards its comparison rather than taking it bare.

The end-to-end economic check is not here — it needs a large corpus and several
seconds, so it lives in `examples/survey.rs`, which asserts.
