# The app that did not move

**2026-11-09.** Why the device app did not follow a finger and fell behind the
page, on main at `041258a73b` with the session's uncommitted work: a Galaxy S23
(Adreno 740, Android 16) and an iPhone 13 mini, Release builds. One run of each
unless a count is given.

## How it was run

- The report: a Release with `PXL_PERF_DRIVE=1`, through `script/perf/`:
  `android-app.py`, `ios-app.py` without its uninstall, and `android-chrome.py`
  on `http://localhost:4173/` from `PXL_PERF_DRIVE=1 make preview`. The reports
  are under `report/`.
- A system trace beside it, then queried with Perfetto's `trace_processor` and
  `tool/stalls.sql`, `tool/input.sql`:

  ```bash
  adb shell perfetto --background -o /data/misc/perfetto-traces/x.pftrace -t 100s \
    -b 256mb sched freq idle am wm gfx view input hal binder_driver -a me.plei.pixelet
  ```

- Touches: a Release without the variable, twelve drags under a 28 s trace, then
  idle: `adb shell input swipe 150 1100 930 1100 1500`, and back.
- The iPhone's memory: `xcrun xctrace record --device <id> --template 'Activity
  Monitor' --all-processes --time-limit 80s` over a measured launch, read from
  `sysmon-process`'s `memory-physical-footprint`. Six launches of the plain
  Release, each looked at after 30 s, and the device's crash logs before and after.

## The drive on the S23

`before` is the stream held across the present; `after` is the stream back
before it. `Chrome` is the same tree and the same drive in the phone's Chrome.

| | before | after | Chrome |
| --- | --- | --- | --- |
| Frames in 20 s | 441 | 256 | 260 |
| `stream_busy` | 172 | 1 | 0 |
| Drawn | 269 | 255 | 259 |
| fps as the report counts | 22.1 | 12.9 | 13.0 |
| Interval p50 / p95 / max, ms | 34 / 142 / 252 | 58 / 209 / 300 | 38 / 263 / 555 |
| GPU p50 / p95, ms | 60.1 / 189.3 | 58.9 / 203.4 | 53.5 / 204.5 |
| Refresh as measured, ms | 22.1 | 8.7 | 16.6 |
| Turns | 3,826 | 3,938 | 677 |

A frame put off as `stream_busy` still counts as a frame in the report's fps.

| System trace | before | after |
| --- | --- | --- |
| `QueuePresentKHR` count | 154 | 239 |
| `QueuePresentKHR` avg / max, ms | 34.6 / 219.1 | 41.3 / 243.0 |
| Main thread `postAndWait` max, ms | 224.8 | 4.5 |
| Main thread `postAndWait` over 16 ms | 29 | 0 |
| RenderThread `dequeueBuffer` max, ms | 210.1 | 0.1 |

The `before` trace holds no present in the window's first 7 s, so its count is
low; its waits are those of the presents it holds. The wait is the `waitForever`
under `queueBuffer`, Android's `Throttling EGL Production`.

## Touches on the S23

| | presents a second |
| --- | --- |
| Drags, the scheduler's old branch | 0.5 (10 in 28 s) |
| Drags, every send a wake | 30–40, max `QueuePresentKHR` 27.4 ms |
| No touch, 20 s later | 13–17 |

Before, 2,135 `topTouchMove` reached JavaScript in the same trace, so the touches
arrived and no frame followed them.

## Turns only at grants, and at the display's frames

A send no longer kicks a turn of its own, a device's completion asks the
scheduler for one, and async pipelines are compiled off the stream. Three drive
runs in a row of that build read 1,159 turns where 3,938 were before and the page
reads 677, and GPU p50 69, 93 and 139 ms as the phone heated (`report/s23-turns.txt`,
`-2`, `-3`), so those runs compare nothing else.

A turn then waits on the core's thread for the display frame the scheduler
offered ahead of it, from the platform's display link. Under the drags, 30 presents
a second at 38–40 °C, with `QueuePresentKHR` at most 8.7 ms and no `postAndWait`
over 16 ms. With no touch it stays at 28–30 a second.

The drive on the last build, two runs from 43 °C to 48 °C: the refresh read 8.3 ms and the pace 16.7; 258 and 238 frames drawn, 317 and
323 put off as `stream_busy`; GPU p50 61.1 and 66.0 ms; 931 turns in the first.
Touches on that build at 43 °C: 25–29 presents a second under the drags, 37–46 after
them, `QueuePresentKHR` at most 37.9 ms and no `postAndWait` over 16 ms.

A first run of that drive ended in `Queue::submit` refusing a destroyed buffer: a
write staged in the stream and a `DESTROY` after it. The writes staged so far now
reach the queue before a destroy, and the two runs above kept going.

## The iPhone

| | run 1 | run 2 |
| --- | --- | --- |
| fps | 17.5 | 17.5 |
| GPU p50 / p95, ms | 51.3 / 105.0 | 52.1 / 104.2 |
| Interval max, ms | 218 | 250 |
| `stream_busy` | 0 | 0 |

The footprint stood at 947 MiB 4 s in and at 1.02–1.04 GiB through 80 s. The six
plain launches were alive after 30 s, and no crash or Jetsam record was added.

## A matrix in a uniform

`Resolve` was declared for the run as `Propagate` is, two `mat4x4f` and then a
`vec4f`, its host writing known values, and the screen pass drew green where
every word read back as written and red where any did not. Green on the S23 in
the app (naga to SPIR-V) and in Chrome (Tint). The real `Propagate`, a compute
pass, was not probed.

## What this does not show

- The page under touches, on either phone, and the iPhone's Safari.
- The iPhone under touches, which nothing here can inject.
- The latency from a touch to the screen, on the app or the page.
- The footprint without the Metal patch, measured the same way; the 2.4 GB
  before it is the kernel's log at the kill.
- What kept the measuring drive drawing while touches drew nothing.
- What wakes the app 13–46 times a second with no touch.
- The last build on the iPhone, which was locked.
- Why the Release in the simulator stands at "Waiting for the server…", where
  its Safari opens the site.
- The cause of the spans' differences between the app and Chrome.
