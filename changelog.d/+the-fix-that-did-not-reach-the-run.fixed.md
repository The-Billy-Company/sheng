Both workflow fixes in the previous entry landed correctly and neither one reached the run it was
written for, for a different reason each.

`miri.yml` stopped aborting and started taking hours. The `cfg(miri)` answer for `CPUID` let the
leg get past `arch::available` and on to `selectivity::tests`, whose three cases all build a
`regex-automata` DFA through the module's own `quotients` helper — the exact cost the job's own
header says it excludes. The skip list named one of the three by prefix, so two ran under the
interpreter, and the leg went from 87 seconds to still-running with nothing failing. The skip is
now the module, and the job carries `timeout-minutes: 20`, so the next test that grows a
determinization is a red job in twenty minutes rather than a queue slot held all afternoon.

`release-please.yml`'s fold job stopped dying on the push and started not running. It was gated on
release-please's `pr` output, which is reported only by the run that creates or updates the PR: the
push carrying the push fix changed nothing release-please acts on, so the output was empty, the job
was skipped, and the open PR kept sitting red on the stale `Cargo.lock` this job exists to move —
the failure mode the gate's own comment says it prevents, since a fold has to be able to re-run
while the PR stays open. When the output is empty the job now asks for the open PR release-please
labels `autorelease: pending` and folds onto that.
