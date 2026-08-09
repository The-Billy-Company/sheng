Two workflows outside `ci.yml` were failing while `ci.yml` was green, which is the arrangement
that lets a red badge stop meaning anything.

`miri.yml` has been red since the AVX-512 probe landed, and not because of anything it found.
Miri does not implement `CPUID` and aborts on it, so `arch::available` took the whole run down
from inside a calibration test. `x86::leaves` now answers `0` under `cfg(miri)` — the honest
answer for an interpreter where no leaf is readable — and because every caller gates its own
`__cpuid` behind a `leaves()` comparison, that one `cfg` disarms the entire probe and leaves
the leg on the scalar path its name promises.

`release-please.yml`'s fold job checked out with `persist-credentials: false` and then pushed
with `git push origin`, which authenticates with the credential that flag exists to withhold.
It passed a `token:` to the checkout as though that supplied one, so the job would fold the
changelog, commit it, and die on the push — visible only once there were fragments to fold,
since an empty fold exits before pushing. The push now carries the App token in a one-off
remote URL, and the branch name and token both arrive through the environment rather than
interpolated into the shell.
