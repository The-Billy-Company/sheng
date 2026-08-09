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

Running on every push is what exposed the fold as incremental. towncrier refuses to write a second
section for a version it has already written, so the second run over an already-folded branch was
a hard error rather than a no-op — and fragments that reached main after the first fold could not
have been folded at that version even if it had been. The job now restores `CHANGELOG.md` and
`changelog.d/` from main before folding, which makes the result a function of main at that version:
idempotent when nothing moved, complete when something did. Its "nothing to fold" check compares
against `HEAD` rather than the index, since restoring those two paths stages them and the previous
comparison would have called a real change quiet.
