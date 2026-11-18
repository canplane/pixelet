# The deployed build's pace, measured

**2026-11-16.** The change of
[the deployed build's pace, taken back](../../minutes/20261116-the-deployed-builds-pace-taken-back.md)
(`deb1fbb08c`) measured against the deployed build #98 (`ac321a51af`) on the six
platforms the app runs on, and against the tree just before it (`ab46932e8b`, #102's
tree) on the S23, where the change alone differs.

## Question

1. Is HEAD at #98's performance on every platform?
2. What does the change alone do, with everything else held?
3. Where HEAD and #98 differ, what makes the difference?

## Builds

| label | commit | what it holds |
| --- | --- | --- |
| #98 | `ac321a51af` | the deployed build of 2026-11-10 |
| control | `ab46932e8b` | the tree before the change, the same as #102's `040487cc5a` |
| HEAD | `72226f95af` | the change (`deb1fbb08c`) and the minutes' notes |

All release builds with `PXL_PERF_DRIVE=1`, each tree built apart (a worktree under
`tmp/` and its own cargo target directory):

- **The web:** `script/build-number.py`, then `pnpm --dir ts/client build:web`, the
  wasm in cargo's `release` profile, served by `node script/web.ts serve` on its own
  `PXL_PREVIEW_PORT`, the tiles from the default source.
- **The apps:** Android `./gradlew :app:assembleRelease` (#98's `mobile` flavor), iOS
  `xcodebuild -configuration Release`, the core by `core-build.py` in cargo's `native`
  profile.
- #98's Android core stopped in `number-paths.py`, whose source roots of that day do not
  hold cargo's git checkouts (the wgpu fork). Its worktree took `/git/checkouts/` as a
  source root, which changes only the path strings numbered in the binary.

## Platforms

| platform | device | OS | browser or app |
| --- | --- | --- | --- |
| S23 Chrome | Galaxy S23 (SM-S911N, Adreno 740) | Android 16 | Chrome 157.0.8103.78 |
| S23 app | the same | the same | the app, wgpu on Vulkan |
| iPhone Safari | iPhone 13 mini | iOS 27.0.1 | Safari 27.0.1 |
| iPhone app | the same | the same | the app, wgpu on Metal |
| Mac Chrome | MacBook Pro, M1 Max, 3456 × 2234 | macOS 27.0.1 | Chrome 157.0.8103.51 |
| Mac Safari | the same | the same | Safari 27.0.1 |

The S23 reached the Mac's pages through adb's reverse, the iPhone through two
cloudflared quick tunnels, one a build.

## Procedure

- **One scenario.** `perf/drive.ts` is the same in #98 and HEAD but for renames: `res`
  fixed, shadows on, the panel on, dawn, the orbit through obstacles; 2 s to settle,
  a 20 s window of turns.
- **Each build by its own day's scripts.** #98's report and protocol predate the
  progress lines HEAD's scripts read, so a HEAD script waits on #98 until its 240 s
  limit. #98 ran under `script/perf/` at `ac321a51af`, the control and HEAD under
  HEAD's. Both write the same `values.json` fields (`build`, `longest`, `gpu`, `tick`).
- **Order.** A warm-up run of the base, dropped, then ABBA rounds
  ([measure](../../gpu/lesson/measure.md), Runs drift), but for the iPhone Safari set
  in the results, which ran by turns, #98 first, with no warm-up. Before each run the
  S23 waited until its AP read 38.5 °C or less; the iPhone rested 180 s; the Mac did
  not wait.
- **The figure.** The median (the lower of two middles) of each side's runs.
- **The clock.** HEAD's scripts read the S23's GPU clock and use every capture; #98's
  read none. One pair more was run with a sampler beside the run (kgsl's
  `gpu_busy_percentage` and `devfreq/cur_freq` every 2 s), and the Mac Safari pair with
  WebKit's processes' RSS every 5 s.

## Threats to validity

- **The S23's clock moves the GPU's time.** Under load it read 348 to 719 MHz from run
  to run, and the same tree (#102's) read GPU p50 52.2 ms in another session's run that
  morning and 79.5 here. A GPU time on the S23 compares only beside its clock (below).
- **#98's runs read no device state.** Its temperatures and clocks in the ABBA sets are
  unknown.
- **Fields whose meaning changed.** The control's `paced` is one frame's gap, #98's and
  HEAD's the paced interval; #98's `refresh` is 64 rAF gaps, HEAD's the low quartile of
  16 vsync timestamps' gaps. `tick` in Safari is read to 1 ms.
- **Lost runs.** Three of HEAD's iPhone Safari runs ended at 50 to 67 s with
  safaridriver's session gone and nothing said: the phone's auto-lock at a minute locks
  a private tab, which then waits for the passcode. Neither build holds a wake lock;
  #98's seven runs, under that day's scripts, all finished. They are left out.
  One #98 Mac Safari run did not reach safaridriver. One HEAD S23 app run is under
  "One S23 app run" below.
- **A disturbed set.** A second iPhone Safari ABBA set read #98's warm-up and two of
  its runs at GPU p50 129 to 194 ms and 106 to 124 frames, HEAD's one run at 50 ms,
  where the first set had read #98 at 50 to 54. It began right after the iPhone app's
  seven runs, about 25 minutes of drawing with 180 s rests, so the likeliest reason is
  a phone still hot and throttled; the iPhone reads no temperature or clock, so this is
  not known. It was stopped and is left out.
- **Earlier S23 Chrome sets.** Two #98-against-HEAD sets ran AB by turns. The first
  waited for nothing: HEAD's second and third runs began at an AP of 53.9 and 55.6 °C,
  hot from the runs before, and held the GPU at 348 MHz the whole window (its first run
  680), so their GPU p50 doubled to 109 ms and their frames fell to 162 to 170; #98's
  runs in it fell 303, 257, 217 frames as the phone warmed. The second set cooled each
  run to 38.5 °C but kept #98 first in every pair. They are kept in the appendix as
  what led to the cooled ABBA runs, not as a result.

## Results

Medians; GPU and tick in ms, frames in the 20 s window.

| platform | build | runs | frames | GPU p50 / p95 | tick p50 / p95 | longest frame |
| --- | --- | --- | --- | --- | --- | --- |
| S23 Chrome | #98 | 4 | 263 | 55.8 / 156.2 | 4.8 / 8.7 | 382 |
| | HEAD | 4 | 236 | 68.1 / 140.2 | 4.4 / 9.2 | 222 |
| S23 Chrome | control | 4 | 213 | 79.5 / 157.4 | 5.1 / 9.8 | 241 |
| | HEAD | 4 | 230 | 69.9 / 135.1 | 4.8 / 9.4 | 203 |
| S23 app | #98 | 3 | 254 | 72.7 / 151.8 | 1.93 / 3.19 | 228 |
| | HEAD | 2 | 287 | 62.9 / 143.0 | 1.51 / 2.76 | 0 |
| S23 app | control | 3 | 300 | 61.6 / 131.1 | 1.57 / 3.17 | 0 |
| | HEAD | 3 | 307 | 58.7 / 129.7 | 1.74 / 2.59 | 0 |
| iPhone Safari | #98 | 3 | 274 | 50.8 / 92.2 | 3 / 4 | 0 |
| | HEAD | 1 | 275 | 49.7 / 92.8 | 3 / 4 | 0 |
| iPhone app | #98 | 3 | 372 | 50.2 / 92.5 | 0.77 / 2.16 | 50 |
| | HEAD | 3 | 376 | 48.8 / 89.5 | 0.73 / 2.30 | 0 |
| Mac Chrome | #98 | 3 | 898 | 20.3 / 28.6 | 1.7 / 2.0 | 56 |
| | HEAD | 3 | 891 | 20.2 / 27.6 | 1.7 / 2.0 | 0 |
| Mac Safari | #98 | 2 | 601 | 21.3 / 31.6 | 2 / 2 | 0 |
| | HEAD | 3 | 597 | 21.2 / 31.3 | 2 / 2 | 0 |

The pixels drawn were the same within each platform: 2.1 Mpx on the S23's and the
iPhone's Chrome and Safari, 2.5 on the S23's app, 2.7 on the iPhone's, 8.3 in the Mac's
Chrome and 7.9 in its Safari. The S23's app read `refresh` 8.3 and `paced` 16.7 on both
builds, its Chrome 16.7 and 16.7.

### The S23's GPU beside its clock

The pair run with the sampler, the drive's window from 50 s:

| | #98 | HEAD |
| --- | --- | --- |
| GPU clock under load, MHz | 719 throughout | 615 to 680 |
| GPU use | 84 to 91% | 79 to 88% |
| GPU p50 / p95, ms | 52.8 / 151.6 | 57.7 / 122.0 |
| cycles a frame at p50 (ms × MHz) | 37,960 | 36,990 |
| frames | 301 | 276 |
| AP before the run, °C | 34.9 | 38.4 |

Each pass's p95 as cycles, HEAD's over #98's at the two clocks (641 MHz the mean of
HEAD's): `world` 0.98, `cast` 1.03, `beam` 1.07, `sample` 1.05, `hole` 1.03,
`light_sample` 0.98, `light_beam` 1.09; `light_hit` 0.18, `light_walk` 0.59; and
`light_raster` 1.38, `light_beam_hit` 1.30, `light_draws` 1.27.

### Refused frames on the S23's app

At the window's end, control: none refused by admission, about 2,500 by the stream;
HEAD: 200 to 380 by admission, about 750 by the stream.

### Mac Safari's memory

WebContent rose from about 300 to 400 MiB across the run alike on both builds; the GPU
process held 280 to 380 MiB on #98 and 280 to 330 on HEAD.

## What the differences come from

- **The change alone (control against HEAD).** On the S23's Chrome every pass's p95 was
  the same or lower, the tick lower at both percentiles, the longest frame shorter (241
  to 203 ms) and the GPU p50 lower (79.5 to 69.9) within the S23's drift. On its app the
  stream refused a third as many frames, as the 60 Hz cap and the frame's admission
  meant, and the tick's p95 fell (3.17 to 2.59) while its p50 rose 0.17 ms, with fewer
  entries each holding the restored budget. The change touches no WGSL.
- **The S23's GPU against #98 is its clock.** At the same use the governor held #98 at
  719 MHz and HEAD at 615 to 680; a frame's cycles are 3% fewer on HEAD, and its passes
  cost the same per cycle but the lights'. Why the governor chose a lower clock was not
  traced; the pair also began 3.5 °C apart.
- **The lights' passes changed between #98 and the control.** `light_hit` and
  `light_walk` fell to a fifth and three fifths, which took the tail (#98's p95 and
  longest frame) down; `light_raster`, `light_beam_hit` and `light_draws` rose by a
  quarter to two fifths. Fifteen commits touched the WGSL between them (164 shader
  files); which of them each change came from was not traced.
- **Elsewhere HEAD is at #98.** The iPhone, both ways, and the Mac, both browsers, read
  within a run's spread of #98.
- **The display read past a frame.** HEAD's one iPhone Safari run read `refresh` 58 ms
  (#98 18 to 31), and both builds read 28 to 32 in the Mac's Safari, whose Chrome read
  7.4 to 8.2: every frame outlasted one display interval, the case the minute left open.
  In the Mac's Safari the GPU did not set the pace: its p50 was 21 ms against frames
  about 33 ms apart. What the code allows (`frame.rs` `frame_task`, `sys/sched`): the
  next vsync is asked only once the frame has run, so a frame that starts late or runs
  long makes the next answered vsync two intervals on, and a display every frame does
  so reads two; the pace's target and the entry's budget take that interval, so an
  entry may run to 85% of two intervals past the frame's start, past the real next
  vsync, whose answer the frame task takes only when the entry yields; then the frame
  runs and asks again late. While the entries have work to fill the budget, the misread
  holds itself. That is not what held the Mac's Safari at 30 frames a second: with the
  web host running an animation frame loop and giving the gap between frames it saw
  back to back, that gap read 31 to 32 ms there too ("Then: the night's quick
  comparison" below). Safari ran the dedicated worker's animation frames at about
  30 Hz; why (a low-power mode, a worker's limit) was not read. #98 read the same: its refresh was read off the animation
  frames by xpute's host pacing, which the user kept in xpute on 2026-11-05
  ([xpute not yet for other programs](../../minutes/20261105-xpute-not-yet-for-other-programs.md));
  `a8e1547d6e` deleted that runtime and paced in the core by vsyncs skipped to a whole
  number of measured intervals
  ([xpute as objects](../../minutes/20261112-xpute-as-objects-references-and-methods.md)).

## One S23 app run

HEAD's third run against #98 began at an AP of 38.5 °C. Its world was shown at about
43 s, and it drew 15 to 25 frames a second until about 80 s. Its captures of the GPU:
70% at 615 MHz at 50 s, 99% at 220 MHz at 61 s, 63% at 220 MHz at 73 s, then 97 and
99% at 615 MHz at 84 and 96 s. From about 80 s it drew two frames a second: frames
went 655, 682, 700, 712, 722 at 79, 84, 90, 95 and 101 s, the frames refused by
admission (the frame latency's frames unfinished) from 560 to 1,113, and by the stream
from 17 to 387, so the GPU, not the core, held each frame for about half a second.
The capture at 72 s shows the ground and the character; the one at 94 s the sky
alone, the panel reading 5.4 fps. The app answered `stands at 38.1155, 128.4622`,
its view's pivot about 500 m from where the drive stands (`perf/measure.ts` refuses a
run whose view ends elsewhere).

What is known: the GPU was saturated, at a clock that read 220 MHz once while 99%
busy, and the view's pivot moved off the ground it began on. What is not: why a frame
of the sky took the GPU half a second, why the pivot moved, and whether the two are
one cause. HEAD's other five app runs and the control's four did not; it was not
reproduced, and not traced.

## How it went

1. `gate.py android-chrome` on #98 and HEAD ended in #98's first run: #98's app answers
   no progress line, and HEAD's script waits for one until its 240 s limit. From then on
   each build ran under its own day's scripts, by a script outside the tree that runs
   them by turns and takes the medians as `gate.py` does.
2. The first S23 Chrome set, uncooled, read HEAD's GPU p50 at twice #98's in two runs;
   their captures showed the clock at 348 MHz against 680 in HEAD's first run. Each run
   then waited for the AP to fall to 38.5 °C.
3. The cooled AB set still read HEAD's GPU p50 above #98's while its passes' p95 read
   alike. The control was built and run against HEAD by ABBA to hold everything but the
   change, after the S23's own record (the lesson above) said runs drift, the first of a
   pair runs faster, and a pass's p95 moves up to twice between loads. The control read
   the change as no worse on every figure.
4. The other platforms ran, each ABBA after a warm-up: the iPhone app, the S23 app
   against #98 and against the control, the Mac's Chrome and Safari.
5. The cooled ABBA set of #98 against HEAD on the S23's Chrome read HEAD's GPU p50 above
   #98's in all four rounds. It was read then as a regression between #98 and the
   control, in the shaders, and the iPhone Safari's lost runs as a fault of HEAD's.
6. Two pairs with a sampler beside them settled both: on the S23 the clock (above), and
   on the Mac's Safari WebKit's memory, alike on both builds.

## What was supposed and dropped

- **The change made the S23's GPU slower.** The control against HEAD read every pass the
  same or lower.
- **The shaders since #98 made it slower.** At the same clock the passes cost the same
  but the lights', which trade between them, and a frame's cycles are 3% fewer.
- **The web belt grew and WebKit killed the page.** Read in the change's diff alone:
  the pool is three chunks of 1 MiB (`FRAME_LATENCY` 2 plus 1), a chunk is made beyond it
  only where none is free and destroyed once its mapping comes back, so no more than the
  frames in flight are made; the guest's packing and the web host's agree, row padding
  and every submit within a run included; the link reuses its frames and makes its views
  again only when the guest's memory is another buffer, and drains the later answers it
  held in the next microtask. Mac Safari's WebKit memory moved alike on both builds.
- **A submit before `QUEUE_DONE` cost each run a chunk more.** The change submits the
  staged writes before it on the web as the native host already did, but the
  renderer asks for no `QUEUE_DONE`.
- **HEAD's page crashed on the iPhone.** Its lost runs stopped at the phone's auto-lock,
  with the private tab waiting for the passcode; the drawing until then was whole
  (status 1, no error, at most one frame unfinished), and HEAD's run that the phone did
  not lock finished at #98's figures.
- **A carry that throws leaves the held answers undrained.** In the link's TypeScript
  End, `task_return` schedules the drain only as the first answer is held. But the
  worker's `link_in` is wrapped to trap the link and answer EPIPE where the guest traps
  (`xpute/runtime/web/src/worker.ts` `entry`), so a carry never throws, and once the
  link is trapped every held answer is carried as EPIPE without crossing.

## Appendix: every run

GPU and tick in ms; the clock is the S23's at the captures with its GPU 80% busy or
more; the AP at the first capture, about 10 s into the run (each S23 run of the cooled
sets began at 38.5 °C or less). #98's runs read neither.

**S23 Chrome, #98 against HEAD, ABBA**

| run | frames | GPU p50 | GPU p95 | tick p50 | tick p95 | longest | AP | clock, MHz |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| warm (#98) | 249 | 65.9 | 153.8 | 5.2 | 9.4 | 428 | | |
| #98-1 | 263 | 55.8 | 164.6 | 4.6 | 8.6 | 332 | | |
| #98-2 | 256 | 58.4 | 175.2 | 5.0 | 9.0 | 382 | | |
| #98-3 | 263 | 57.3 | 151.4 | 4.9 | 8.7 | 449 | | |
| #98-4 | 288 | 53.3 | 156.2 | 4.8 | 9.0 | 382 | | |
| HEAD-1 | 236 | 70.2 | 140.2 | 5.1 | 9.6 | 230 | 54.1 | 719, 550, 550 |
| HEAD-2 | 239 | 68.1 | 144.3 | 5.1 | 9.2 | 263 | 43.6 | 550, 550 |
| HEAD-3 | 236 | 70.0 | 147.6 | 4.4 | 9.7 | 222 | 53.4 | 550, 475 |
| HEAD-4 | 277 | 59.4 | 120.3 | 3.8 | 9.0 | 210 | 54.5 | 680, 615 |

**S23 Chrome, control against HEAD, ABBA**

| run | frames | GPU p50 | GPU p95 | tick p50 | tick p95 | longest | AP | clock, MHz |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| warm (control) | 185 | 94.8 | 174.5 | 5.9 | 9.8 | 238 | 49.2 | 401 |
| control-1 | 170 | 109.1 | 159.2 | 5.2 | 10.0 | 241 | 51.2 | 401, 348, 348 |
| control-2 | 227 | 75.3 | 148.6 | 5.1 | 9.8 | 173 | 53.3 | 475, 475 |
| control-3 | 213 | 79.5 | 157.4 | 4.9 | 9.8 | 299 | 55.7 | 475, 475 |
| control-4 | 215 | 79.8 | 165.0 | 5.5 | 9.6 | 282 | 54.3 | 475, 475 |
| HEAD-1 | 269 | 59.6 | 134.4 | 4.3 | 8.9 | 203 | 53.6 | 719, 680, 615 |
| HEAD-2 | 230 | 70.6 | 145.9 | 5.2 | 9.4 | 285 | 55.9 | 719, 550, 550 |
| HEAD-3 | 208 | 80.1 | 152.1 | 5.2 | 9.8 | 234 | 56.4 | 475, 475 |
| HEAD-4 | 230 | 69.9 | 135.1 | 4.8 | 9.4 | 195 | 55.0 | 719, 550, 475 |

**S23 Chrome, #98 against HEAD by turns (AB), not a result**

| set | run | frames | GPU p50 | GPU p95 | tick p50 | tick p95 | longest | AP | clock, MHz |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| uncooled | #98-1 | 303 | 50.8 | 149.6 | 3.6 | 8.6 | 459 | | |
| | #98-2 | 257 | 56.0 | 155.6 | 5.3 | 9.4 | 482 | | |
| | #98-3 | 217 | 62.0 | 174.5 | 4.6 | 9.1 | 721 | | |
| | HEAD-1 | 293 | 54.6 | 112.2 | 4.2 | 9.4 | 200 | 46.2 | 680, 680 |
| | HEAD-2 | 170 | 108.9 | 161.4 | 4.6 | 9.7 | 335 | 53.9 | 348, 348, 348 |
| | HEAD-3 | 162 | 109.1 | 165.5 | 5.0 | 9.9 | 335 | 55.6 | 348, 348 |
| cooled | #98-1 | 256 | 54.8 | 160.6 | 4.9 | 9.2 | 539 | | |
| | #98-2 | 277 | 54.6 | 154.1 | 4.4 | 8.6 | 457 | | |
| | #98-3 | 240 | 58.1 | 167.5 | 5.2 | 8.9 | 460 | | |
| | HEAD-1 | 253 | 63.9 | 127.1 | 4.8 | 9.6 | 208 | 45.5 | 615 |
| | HEAD-2 | 206 | 82.1 | 149.9 | 4.5 | 9.6 | 278 | 54.8 | 475, 475 |
| | HEAD-3 | 209 | 80.7 | 157.4 | 5.7 | 9.7 | 244 | 53.5 | 475, 475 |

**S23 app**

| set | run | frames | GPU p50 | GPU p95 | tick p50 | tick p95 | longest | AP | clock, MHz |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| #98 / HEAD | warm (#98) | 255 | 70.9 | 152.3 | 1.77 | 2.81 | 96 | | |
| | #98-1 | 254 | 69.5 | 147.6 | 1.93 | 2.85 | 253 | | |
| | #98-2 | 254 | 72.8 | 152.3 | 1.89 | 3.32 | 204 | | |
| | #98-3 | 249 | 72.7 | 151.8 | 1.97 | 3.19 | 228 | | |
| | HEAD-1 | 289 | 62.9 | 143.0 | 1.55 | 2.96 | 0 | 42.6 | 615, 615 |
| | HEAD-2 | 287 | 63.0 | 144.6 | 1.51 | 2.76 | 0 | 43.9 | 615, 615 |
| | HEAD-3 | no result: "One S23 app run" | | | | | | | |
| control / HEAD | warm (control) | 338 | 54.3 | 117.3 | 1.78 | 3.00 | 0 | 42.5 | 719, 615 |
| | control-1 | 329 | 54.7 | 114.4 | 1.71 | 2.75 | 0 | 40.2 | 719, 680, 615 |
| | control-2 | 300 | 61.6 | 131.1 | 1.57 | 3.19 | 0 | 47.2 | 615, 680, 615, 615 |
| | control-3 | 295 | 61.6 | 136.8 | 1.52 | 3.17 | 0 | 45.6 | 615, 615 |
| | HEAD-1 | 307 | 58.7 | 129.7 | 1.80 | 2.69 | 0 | 45.0 | 615, 615, 615 |
| | HEAD-2 | 318 | 57.9 | 123.6 | 1.65 | 2.59 | 0 | 42.6 | 680, 615 |
| | HEAD-3 | 296 | 61.7 | 130.6 | 1.74 | 2.55 | 0 | 40.0 | 615, 615 |

**The iPhone, the Mac**

| platform | run | frames | GPU p50 | GPU p95 | tick p50 | tick p95 | longest | refresh / paced |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| iPhone Safari | #98-1 | 268 | 54.1 | 92.2 | 2 | 4 | 0 | 31 / 31 |
| | #98-2 | 285 | 49.9 | 89.2 | 3 | 4 | 0 | 23 / 23 |
| | #98-3 | 274 | 50.8 | 92.9 | 3 | 4 | 0 | 18 / 18 |
| | HEAD-3 | 275 | 49.7 | 92.8 | 3 | 4 | 0 | 58 / 58 |
| iPhone app | warm (#98) | 360 | 51.7 | 93.9 | 0.70 | 2.44 | 56 | 16.7 / 16.7 |
| | #98-1 | 358 | 50.5 | 92.9 | 1.06 | 2.29 | 50 | 16.7 / 16.7 |
| | #98-2 | 372 | 50.2 | 92.5 | 0.77 | 1.99 | 50 | 16.7 / 16.7 |
| | #98-3 | 372 | 50.1 | 91.3 | 0.77 | 2.16 | 0 | 16.7 / 16.7 |
| | HEAD-1 | 376 | 48.8 | 89.6 | 0.72 | 2.48 | 0 | 16.6 / 16.6 |
| | HEAD-2 | 372 | 49.0 | 87.8 | 0.74 | 2.22 | 0 | 16.6 / 16.6 |
| | HEAD-3 | 376 | 48.7 | 89.5 | 0.73 | 2.30 | 0 | 16.7 / 16.7 |
| Mac Chrome | warm (#98) | 900 | 20.3 | 28.6 | 1.7 | 2 | 58 | 8.1 / 16.2 |
| | #98-1 | 902 | 20.1 | 28.6 | 1.7 | 2 | 52 | 7.6 / 15.2 |
| | #98-2 | 898 | 20.3 | 28.0 | 1.7 | 2 | 56 | 7.4 / 14.8 |
| | #98-3 | 888 | 20.4 | 28.6 | 1.7 | 2 | 75 | 8.2 / 16.4 |
| | HEAD-1 | 891 | 20.2 | 27.6 | 1.7 | 2 | 0 | 7.8 / 15.6 |
| | HEAD-2 | 896 | 20.2 | 27.6 | 1.7 | 2 | 0 | 8.2 / 16.4 |
| | HEAD-3 | 878 | 20.3 | 28.5 | 1.7 | 2 | 0 | 8.2 / 16.4 |
| Mac Safari | warm (#98) | 628 | 20.7 | 31.3 | 2 | 2 | 0 | 28 / 28 |
| | #98-1 | 613 | 21.3 | 31.9 | 2 | 2 | 0 | 30 / 30 |
| | #98-3 | 601 | 21.6 | 31.6 | 2 | 2 | 0 | 30 / 30 |
| | HEAD-1 | 601 | 21.2 | 31.3 | 2 | 2 | 0 | 31 / 31 |
| | HEAD-2 | 565 | 21.9 | 32.0 | 2 | 2 | 0 | 31 / 31 |
| | HEAD-3 | 597 | 21.1 | 31.3 | 2 | 2 | 0 | 32 / 32 |

The iPhone Safari set is the first, by turns; its HEAD-1 and HEAD-2 are the lost runs.
The S23 Chrome control's `paced` is one frame's gap and is left out.

## Not measured

The phones' temperature beside #98's runs in the ABBA sets; the iPhone's state; a touch;
memory but on the Mac's Safari; the deployed build itself, which carries no
`PXL_PERF_DRIVE`.

## Kept

The change. The runs were under `tmp/perf/cmp/`, not kept; the figures above are theirs.

## Then: the night's quick comparison (2026-11-17)

The tree measured above (`72226f95af`) against HEAD after the day's further changes
(`6de67851df`: the vsync answering `next(after)` with the frame's gap, a web wait of 0
on a posted message, the location indicator's cells and the markers asked under the
budget). The last of these was taken back:
the location is one memo for the zone, keyed by the fix's leaf cell, and runs as a tick
after the ghost, and the markers ask as before, a brick's shown until it is read again.
The figures below are the taken-back code's. Release web builds with `PXL_PERF_DRIVE=1`, both under HEAD's scripts, a warm-up
and one ABBA round each: the S23's Chrome cooled to 38.5 °C before each run, the Mac's
Chrome, then its Safari.

| platform | build | frames | GPU p50 / p95 | tick p50 / p95 | effects p95 | scene p50 / p95 |
| --- | --- | --- | --- | --- | --- | --- |
| S23 Chrome | 72226f95af | 292, 297 | 53.3–55.2 / 110–113 | 4.1 / 8.8–12.3 | 5.9–7.5 | 2.8–3.3 / 7.4–9.0 |
| | 6de67851df | 306, 307 | 52.0–52.3 / 106–111 | 5.9–6.2 / 11.9–12.1 | 0.2 | 4.3–4.4 / 11.8–12.0 |
| Mac Chrome | 72226f95af | 862–879 | 20.4–20.6 / 28.8–29.3 | 1.7 / 2.0 | 0.9 | 1.2–1.3 / 1.7–2.2 |
| | 6de67851df | 859–862 | 20.3–20.8 / 29.1–29.2 | 1.9–2.0 / 2.7–2.8 | 0.1 | 1.6 / 2.5–2.6 |
| Mac Safari | 72226f95af | 565–579 | 21.4–21.5 / 31.7–32.0 | 2 / 2 | 1.0 | 1.2–1.3 / 2.0–3.0 |
| | 6de67851df | 576–585 | 21.2–21.3 / 32.0 | 2 / 3 | 0.0 | 1.5–1.6 / 3.0 |

Frames and the GPU held or improved; the location effect's spikes (its p95) left the
effects. The tick rose at both percentiles, its scene step about 1.2 ms at p50 on the S23
and 0.3 on the Mac, every frame alike.

**Which commit.** Each build once in an order and once in its reverse, the Mac's Chrome,
the scene step's p50 / p95 in ms: `72226f95af` 1.2/2.2, 1.3/2.3; `bec3061dc4` (the vsync and
the wait of 0 in) 1.3/1.8, 1.3/2.4; `cfc72c4ac5` (the location's cells) 1.5/2.7, 1.5/2.7;
`ff07ca506e` (the markers) 1.6/2.8, 1.6/2.8; HEAD 1.5/2.6, 1.6/3.0. The rise came with the
location's cells; the markers' share is within 0.1 ms.

**What it was not.**
- A clock read for each item of the location's pass, `sched::run` reading `spent()` after
  a step has worked: the location's pass was cut to the cells due alone, and the S23 read
  the same (tick p50 5.9 and 6.2 against 3.9 and 4.0; scene 4.2 and 4.3 against 2.7).
- Each cell's memo tested against the frame's changes: a test of the zone's span first,
  each cell only where a change touched it, and the Mac read the same (scene 1.5 against
  1.2–1.3).
- The step's own cost on a settled world: 18 µs natively, 10.6 of it the list of cells
  made again (a throwaway test).

**Where it was.** A diagnostic build ran the same location code as an effect, timed under
the `setting` effect's slot, and the Mac's Chrome read the base's figures again: scene
1.2 and 1.3, tick 1.7 / 2.0, effects p50 0.1 and p95 1.0. So the cost is the code's in
the visible phase, where the terrain's demand has spent most of the budget before it: its
pass then asks a cell a frame, and the zone is laid again in every frame that asked one,
a zone of some new and some old tops each time. Supposed from that, not traced.

**A toast while the Mac's Chrome ran.** The page showed that the server was hard to
reach, for a moment. The core shows it on one read the client counts as the server's
failure: a status but 200, 204 and 404, or a fetch that threw while online
(`service/sys/fs/read.ts`, `sys/io/policy.rs` `fault`). A read the core stopped is not one:
the scheduler ends its attempt as it stops it, and the late answer finds none
(`request.rs` `settle`). The page ran on `web.ts serve`, which forwards every tile request
to the deployment through Node (`script/proxy.ts` `forward`) and answers 502, unlogged,
when that leg fails; the run reads none of the page's console. A failed leg is the likely
cause; not shown.

## Then: the view's turn and the markers' stand (2026-11-17)

`64e4fae492` against the tree with the markers standing across a brick's split or
merge and the view taking a turn only past half a CSS pixel (`948b7b3889`,
`93a64f5181`). Release web builds with `PXL_PERF_DRIVE=1`, the Mac's Chrome, three
runs each by turns (before, after, after, before, before, after).

| build | frames | GPU p50 / p95 | tick p50 / p95 | scene p50 / p95 |
| --- | --- | --- | --- | --- |
| before | 839–845 | 20.8–21.0 / 29.6–29.8 | 1.8 / 2.6–2.7 | 1.2–1.3 / 1.7–2.2 |
| after | 844–850 | 20.6–21.0 / 29.4–30.0 | 1.8 / 2.6–2.7 | 1.2 / 1.8–2.2 |

No difference past the runs' own spread. The drive moves the camera throughout, so
what the turn's threshold saves at rest is not in these figures; what they show is
that nothing got slower. The phones were not measured.

A second comparison the same day, `64e4fae492` against `9fd850c654` with the places'
asks under the budget laid on it (a form since taken back), three runs each by turns:

| build | frames | GPU p50 | tick p50 / p95 | scene p50 |
| --- | --- | --- | --- | --- |
| before | 855, 686, 741 | 21.0, 26.4, 24.2 | 1.8 / 2.7–2.8 | 1.3 |
| after | 858, 728, 800 | 20.6, 24.8, 22.0 | 1.8 / 2.7–2.8 | 1.3 |

The core's figures are the same. The GPU's rose on both builds from the third run on,
the Mac's own state; within each pair by turns the later build is no slower. The
report's new line of asks by reason read, over a run: method about 2500, view 2100 to
2400, completion 3000 to 3300, pending 1600, animation 1000 to 1100, scene 0.

Three more the same day, each `64e4fae492` against the tree named, by turns:

| against | before: tick p50 / p95, GPU p50 | after: tick p50 / p95, GPU p50 |
| --- | --- | --- |
| the places under the budget (`e77e5f8a03`, before its review's fix) | 1.8 / 2.8, 20.6; 2.0 / 3.8, 29.0; 1.8 / 2.8, 25.6 | 1.8 / 2.8, 20.4; 1.8 / 2.7, 20.6; 1.8 / 2.9, 23.1 |
| the ease ended by the view, its first forms | 1.8 / 2.8, 20.6; 2.4 / 4.8, 31.7; 1.9 / 2.9, 22.7; 1.9 / 3.1, 21.9 | 1.8 / 2.7, 20.6; 1.8 / 2.8, 20.7; 1.9 / 2.9, 24.1; 2.7 / 4.6, 24.3 |
| `d753e720dc` | 1.8 / 2.8, 20.4; 1.8 / 2.8, 20.8; 1.9 / 4.5, 21.2 | 1.8 / 2.8, 21.0; 1.8 / 2.7, 20.6; 2.7 / 4.3, 23.1 |

The first four runs of a series read the same on both builds. A run late in a series
is slower on whichever build it falls to, the tick with the GPU; the scenario does not
turn places on, so the places' asks were not exercised.

## Then: the S23's Chrome on the code of 2026-11-17

#98 (`ac321a51af`) against `03aee8151c`, the day's last, each a release web build with
the drive and run by its own day's scripts; a warm-up of #98 dropped, then two rounds
by turns, each run begun once the AP read 38.5 °C or less (it waited 30 to 151 s, longer
each time, and read 45 to 48 °C after a run).

| run | build | frames | GPU p50 / p95 | tick p50 / p95 | effects p50 / p95 | longest |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | #98 | 288 | 52.2 / 161.3 | 4.1 / 9.1 | 0.1 / 5.7 | 441 |
| 3 | HEAD | 247 | 58.3 / 133.9 | 5.9 / 13.0 | 0.1 / 0.2 | 168 |
| 4 | HEAD | 220 | 76.8 / 147.2 | 6.1 / 13.2 | 0.1 / 0.2 | 178 |
| 5 | #98 | 212 | 77.6 / 208.3 | 4.8 / 9.2 | 1.9 / 5.9 | 448 |
| 6 | #98 | 207 | 77.8 / 207.4 | 5.2 / 9.8 | 2.2 / 6.1 | 434 |
| 7 | HEAD | 174 | 96.3 / 178.5 | 7.0 / 14.1 | 0.1 / 0.2 | 320 |
| 8 | HEAD | 174 | 98.3 / 180.6 | 6.8 / 14.0 | 0.1 / 0.2 | 246 |
| 9 | #98 | 198 | 81.9 / 216.0 | 5.5 / 9.3 | 0.1 / 6.1 | 439 |

- **The tick is not the same sum on the two builds.** #98 runs the location indicator
  as an effect, outside the tick; HEAD runs it as a tick (`interaction/location`, 0.9 to
  1.7 ms a run, 9 to 10 at most). The effects' p50 and the tick's, added, read 4.2,
  6.7, 7.4 and 5.6 ms on #98's runs and 6.0, 6.2, 7.1 and 6.9 on HEAD's: higher on
  HEAD in two of the four pairs and lower in two. At p95 the tick alone reads 9.1 to 9.8
  on #98, beside effects of about 6, and 13.0 to 14.1 on HEAD with the location in it.
- **Frames:** #98's four mean 226, HEAD's 204, a tenth fewer, as on 2026-11-16 (263
  against 236). The phone slowed through the series on both builds: #98 from 288 frames
  to 198.
- **The GPU:** its p50 follows the phone's clock, which HEAD's runs read between 348
  and 719 MHz in the window, lower in the later runs; #98's scripts read none. HEAD's
  p95 and its longest frame are the lower in every pair.
- The location's zone is worked out on most frames of HEAD's every run and of two of
  #98's (its effect's p50 1.9 and 2.2 ms; 0.1 in the other two), where a zone at the
  same cell was expected to ask nothing.

### The location's zone, laid only while drawn

Read in the code: the drive's fix stands at the camera's pivot, 755 units from the
eye, where neither the zone nor the beacon is drawn; the ground's leaf is not held
there, so the grounding's answers are coarser ones, and a memo of answers not exact is
dropped by any ground node landing over its columns, about ten a second through the
drive. Each drop grounded the whole zone again, tens of queries.

`8cb606094e` grounds the center alone each step and lays the cells only in the frames
that draw the zone. `03aee8151c` against it, a warm-up dropped, two rounds by turns,
each run begun at 38.5 °C or less:

| run | build | frames | GPU p50 / p95 | tick p50 / p95 | location, a run / most |
| --- | --- | --- | --- | --- | --- |
| 2 | before | 227 | 72.4 / 145.0 | 5.9 / 14.0 | 1.2 / 8.2 |
| 3 | after | 198 | 84.9 / 161.3 | 4.8 / 10.7 | 0.0 / 1.6 |
| 4 | after | 183 | 94.5 / 168.1 | 5.8 / 10.1 | 0.0 / 0.8 |
| 5 | before | 188 | 90.4 / 179.2 | 6.2 / 13.1 | 0.9 / 7.2 |
| 6 | before | 172 | 102.3 / 164.7 | 7.1 / 13.9 | 2.1 / 10.0 |
| 7 | after | 197 | 83.6 / 172.4 | 5.0 / 11.4 | 0.1 / 0.8 |
| 8 | after | 182 | 95.4 / 170.8 | 4.9 / 11.0 | 0.0 / 0.3 |
| 9 | before | 177 | 97.2 / 168.6 | 6.2 / 14.1 | 1.2 / 8.8 |

The location's step fell from about a millisecond a run to none, and the tick with it:
its p50 from 5.9 to 7.1 down to 4.8 to 5.8, its p95 from 13.1 to 14.1 down to 10.1 to
11.4, against #98's 9.1 to 9.8 with its effects of about 6 beside. The frames and the
GPU moved with the phone, which slowed through the series as before; the measured
build was the change before its review's last fixes, which touched a test and a
comment.

## Then: a pan past the service area

The Mac's Chrome, a release web build without the drive, the app's own profiler; the
view past the service area (the East Sea), at the orbit's largest distance.

| | the companions' step, most | the input's step, most | tick p95 / most |
| --- | --- | --- | --- |
| HEAD before, one drag (43.9°N 145.2°E, 131072 u, polar 27°) | 217.5 | 98.8 | 99.3 / 217.8 |
| HEAD before, one drag (37.4°N 133.2°E, about 106,000 u, polar 60°) | 14.5 | 68.1 | 9.9 / 68.6 |
| the deployed build (`34cfa83`), the same place and drag | 13.9 | 96.6 | 13.4 / 97.3 |
| `18598cc6e8`, three drags (37.4°N 134.5°E, the largest distance) | 0.2 | 0.1 | 0.6 / 0.8 |

In ms. Before, the two steps had read 561 and 541 ms at most in the same session; a
wheel's zoom there cost nothing. The GPU read 13 to 14 ms at p95 throughout.

The cause, read and timed natively: `World::slide` walked one leaf cell a step, at
about 5 µs a step (10,000 by 5,000 cells: 72.7 ms; 100,000 by 50,000: 773 ms), and past
the area no leaf is lacking, so the walk ran its whole way; over served ground far
out it ends at the first leaf not held. After `18598cc6e8` both walks take 0.07 ms.

Met on the way and not acted on: over a void leaf tile inside the area a support query
runs the ray to its cap of steps (about 0.77 ms a query); and a ray that ends at the
cap is read as having met nothing, which a lift reads as filled to the world's top
(`floor_under` from z = -2100 over flat ground at 5 answered 32768).

Later the same day `18598cc6e8` was taken back and the walk closed where it had been
opened (`68f5c3828e`, `e88d2fa7d3`): a walker stands where a decor of its cell is
drawn, out to 192 units from the eye, and a walk is walked only where it stands at
both its ends, so it is under 384 leaf cells an axis; past that a pan walks nothing,
and reads the surface once where the pivot's height is not known. The same native
walk of 27 leaf cells took 302 µs before and 279 after the floor's fix beside it
(`c02a80d07b`). Not measured in the app.

The ray that ended at the cap and was read as filled to the world's top is closed
by the rule every other ray reads by: the climb's rays read by their distance. Native,
`floor_under` over flat ground:

| inside solid by | fixed at the finest, before | by distance |
| --- | --- | --- |
| 100 cells | 0.13 ms | 0.14 |
| 2,000 | 0.84 | 0.18 |
| 5,000 | 32768 answered | 0.26 |
| 30,000 | 32768 answered | 0.35 |
| 2,000, something placed at the finest zoom in the cell | 56.6 | 10.8 |

The near walk of 27 leaf cells takes 333 to 345 µs where it took 302 before the day
and 279 with the fixed detail: a ray by distance asks a node over it for the bound of
every zoom asked within it (`ground::node`, `Resident::relief`), where a ray at one
zoom asks one. That is the one rule's price where the fixed zoom was fitted to the
near case, and it bought the far one's wrong answers.

With places on, the same build, a wheel out twenty steps and back sixteen: the scene's
step 0.8 ms a run and 5.6 at most, the tick 1.4 / 2.6 and 6.4 at most, no long frame.
