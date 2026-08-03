The `survey` gate used to read the clock's own resolution as a verdict about the
cost model. It timed each arm once as a min-of-five and asserted that every armed
row cleared 1.000x, which is the right claim over a real corpus and nonsense over
a small one: run against this crate's own 0.3 MiB of source, three rows came out
at 0.62-0.95x and the assertion announced that the model admitting them was
wrong. The same three rows measure 1.07-1.50x over 22.6 MiB. Nothing was wrong
with the model; the instrument was reporting noise and a cache-resident corpus as
evidence.

Two changes, both narrowing what the gate is willing to claim rather than what it
enforces. Every arm is now five samples of a min-of-five, so a row carries an
interval instead of a number, and only a row whose whole interval sits below
1.000x counts as a loss - one that straddles is printed as undecided and asserts
nothing. And below 8 MiB of corpus the survey declines to judge the model at all,
because a calibration is nanoseconds per byte read from memory and a corpus that
fits in cache never reads from memory, so the engine's own `memchr` accelerator
runs at tens of gigabytes a second and beats every price the crate knows for
reasons that have nothing to do with the sieve.

A row that genuinely loses still fails the run, and now says so legibly - the
ratio, the interval, and both arm times, instead of a debug dump of raw seconds.
