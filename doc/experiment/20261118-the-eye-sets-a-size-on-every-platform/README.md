# The eye sets a size, on every platform

**2026-11-18.** The app's own scenario (`perf/measure.ts`, `perf/drive.ts`) on the
commit the day ended on, `7d8b1f2d19`, where a cell asks at the eye's size no finer than
what is held over it, a body the eye does not draw asks no floor, and a kept answer goes
by the zooms it read. Two measurements, apart: a watch for a regression against the tree
before the day's work, and the commit alone on each platform. What the day decided is in
[the eye sets a size and the world answers](../../minutes/20261118-the-eye-sets-a-size-and-the-world-answers.md).

The commit was built three times as it took shape, and a run's `report.txt` names the
build it ran: `b1bc4c2759` and `11ce19d922` are the commit before its last round, which
are one tree in the Rust; `a017105040` is the commit. The last round made one body's
shown test one point and took a walk's unguarded read of the surface away.

## Procedure

Release builds with `PXL_PERF_DRIVE=1`. The web's bundle of the commit was served on one
port and the bundle of `630c31c8f2`, the tree before the day's work, built in a worktree
with the main checkout's packages, on another. The Android phone reached both by `adb
reverse`, plugged in, its screen kept on. The iPhone reached the commit's bundle through
a tunnel.

| device | model | chip, memory | system | browser |
|---|---|---|---|---|
| the Android phone | Galaxy S23 (SM-S911N) | Snapdragon 8 Gen 2 (SM8550, Adreno 740), 8 GB | Android 16, patch 2026-09-05 | Chrome 157.0.8103.78 |
| the iPhone | iPhone 13 mini (iPhone14,4) | A15 | iOS 27.0.1 | Safari 27.0.1 |
| the Mac | MacBook Pro 16-inch (MacBookPro18,2) | M1 Max, 10 CPU cores (8 and 2), 32 GPU cores, 64 GB | macOS 27.0.1 (26A434) | Safari 27.0.1 (22625.1.29.11.28), Chrome 157.0.8103.79 |

What each run drew and at what interval, from its `report.txt` (`drawn`, `refresh`,
`paced`); the scale was 1.00 in all:

| platform | drawn, Mpx | `refresh` as read, ms | paced, ms |
|---|---|---|---|
| the Mac's Chrome | 8.3 | 16.7 | 16.7 |
| the Mac's Safari | 2.0 (7.9 in its first run) | 22.0 | 22.0 |
| the Android phone's Chrome | 2.1 | 16.7 | 16.7 |
| the Android phone's app | 2.5 | 8.3 | 16.7 |
| the iPhone's Safari | 2.1 | 59.0 | 59.0 |
| the iPhone's app | 2.7 | 16.6 | 16.6 |

The Mac's two browsers did not draw the same picture, so their figures are not set
beside each other. Both scripts ask a window of 1920 × 1080 CSS pixels
(`script/perf/measure.py` `SIZE`); Chrome's forces two device pixels a CSS pixel
(`--force-device-scale-factor=2`) and draws 3840 × 2160, and Safari's takes the scale of
the screen its window opens on, which was one.

```bash
PXL_PERF_DRIVE=1 PXL_PREVIEW_PORT=4175 make preview            # the commit
PXL_PERF_DRIVE=1 PXL_PREVIEW_PORT=4174 make preview            # 630c31c8f2, in its worktree
./script/perf/gate.py safari http://localhost:4174 http://localhost:4175 3
./script/perf/gate.py android-chrome http://localhost:4174 http://localhost:4175 3
./script/perf/chrome.py http://localhost:4175 tmp/perf/record/mac-chrome-1          # and 2, 3
./script/perf/safari.py http://localhost:4175 tmp/perf/record/mac-safari-1          # and 2, 3
./script/perf/android-chrome.py http://localhost:4175 tmp/perf/record/android-chrome-1
./script/perf/android-app.py ts/client/android/app/build/outputs/apk/release/app-release.apk tmp/perf/record/android-app-1
cloudflared tunnel --no-autoupdate --url http://127.0.0.1:4175
./script/perf/ios-safari.py https://<tunnel> tmp/perf/record/ios-safari-1
IOS_DEVICE=<devicectl id> ./script/perf/ios-app.py ts/client/ios/build/derived/Build/Products/Release-iphoneos/pixelet.app tmp/perf/record/ios-app-1
```

`report/` holds each run's `values.json`, `report.txt` and `log.txt`, and each gate's
output; traces and shots are not kept. The tables are read from those files: a step's
figure is its mean a run, the phone's clock and temperature are its run's last capture.

## The watch

### The phone's Chrome

On `57e0fdc072`, the day's tree before this commit, the gate failed, tick p50 past the
base's share: tick p50 4.60 to 6.10, p95 10.90 to 13.30
(`report/watch/android-chrome-before-the-fix.txt`; its runs were not kept). The rise was
read in the code before anything was changed, in the two steps that carried it:

- **The places' surfaces** (the scene step). A place was asked again whenever any size
  its read had recorded re-took differently, and a size that could not change the answer
  was among them.
- **The bodies' floors** (the companion step). Over bare ground a floor was asked again
  at the edges under 48, 24 and 12 units, where nothing finer is held; a body past where
  the eye draws one still read its five floors; and a change to a brick of a zoom the
  answer had not read dropped it.

The fix is the commit, from the rule and no tuning. On it the gate passes, the phone cool
and its GPU clock alike across the six runs: tick p50 3.90 to 4.50, p95 11.40 to 11.70,
frames 312 to 319, GPU p50 51.38 to 51.18, p95 109.77 to 105.32.

| run | build | frames | tick p50 / p95, ms | ticks over budget | GPU p50 / p95, ms | scene, ms | demand, ms | companion, ms | GPU clock, AP |
|---|---|---|---|---|---|---|---|---|---|
| base 1 | `630c31c8f2` | 307 | 3.9 / 12.8 | 99 | 51.4 / 109.8 | 3.0 | 2.0 | 0.5 | 719 MHz, 35.4 °C |
| base 2 | `630c31c8f2` | 312 | 4.1 / 11.4 | 85 | 51.5 / 106.5 | 2.9 | 1.8 | 0.5 | 719 MHz, 41.4 °C |
| base 3 | `630c31c8f2` | 316 | 3.8 / 10.3 | 75 | 51.3 / 111.2 | 2.8 | 1.7 | 0.5 | 719 MHz, 45.6 °C |
| change 1 | `a017105040` | 319 | 4.5 / 12.5 | 102 | 50.5 / 105.3 | 3.3 | 2.0 | 0.3 | 719 MHz, 39.0 °C |
| change 2 | `a017105040` | 319 | 4.5 / 11.7 | 102 | 51.2 / 102.9 | 3.4 | 1.9 | 0.2 | 719 MHz, 43.5 °C |
| change 3 | `a017105040` | 319 | 4.0 / 11.0 | 92 | 51.4 / 106.1 | 3.0 | 1.8 | 0.2 | 680 MHz, 51.5 °C |

The companion step is under the base's. The scene step stays 0.3 to 0.4 ms over it: a
place's ask is a cell's settle where it was one ray, and a place has no gate, since
whether it shows is the answer of its own ask. The gate before the last round, on
`b1bc4c2759`, passed too (`report/watch/android-chrome-b1bc4c2759/`): tick p50 4.20 to
4.40, with the phone's clock moving from 348 to 719 MHz between its runs.

### The Mac's Safari

Run on `57e0fdc072` and not again; the gate passes: GPU p50 15.93 to 15.84, p95 26.71 to
25.48, frames 792 to 825, tick p50 2.00 to 2.00, p95 2.00 to 3.00 (Safari reads a tick
to 1 ms).

| run | build | frames | tick p50 / p95, ms | GPU p50 / p95, ms | scene, ms | demand, ms | companion, ms |
|---|---|---|---|---|---|---|---|
| base 1 | `630c31c8f2` | 778 | 2.0 / 2.0 | 15.9 / 26.9 | 1.2 | 0.4 | 0.1 |
| base 2 | `630c31c8f2` | 784 | 2.0 / 2.0 | 15.9 / 26.7 | 1.3 | 0.4 | 0.1 |
| base 3 | `630c31c8f2` | 800 | 2.0 / 2.0 | 16.1 / 25.6 | 1.2 | 0.4 | 0.1 |
| change 1 | `57e0fdc072` | 823 | 2.0 / 3.0 | 15.8 / 25.5 | 1.5 | 0.4 | 0.2 |
| change 2 | `57e0fdc072` | 825 | 2.0 / 3.0 | 15.7 / 26.2 | 1.5 | 0.4 | 0.2 |
| change 3 | `57e0fdc072` | 825 | 2.0 / 3.0 | 15.9 / 25.2 | 1.5 | 0.4 | 0.2 |

## The commit alone

Three runs a platform, in the order run.

### The Android phone

| run | build | frames | tick p50 / p95, ms | ticks over budget | GPU p50 / p95, ms | scene, ms | demand, ms | companion, ms | GPU clock, AP |
|---|---|---|---|---|---|---|---|---|---|
| Chrome 1 | `b1bc4c2759` | 235 | 4.7 / 11.8 | 79 | 66.3 / 145.0 | 3.3 | 2.2 | 0.2 | 401 MHz, 47.2 °C |
| Chrome 2 | `b1bc4c2759` | 270 | 5.2 / 12.4 | 106 | 62.1 / 120.9 | 3.6 | 2.3 | 0.3 | 615 MHz, 49.1 °C |
| Chrome 3 | `b1bc4c2759` | 265 | 5.2 / 12.6 | 103 | 63.3 / 123.1 | 3.4 | 2.3 | 0.2 | 550 MHz, 49.1 °C |
| app 1 | `b1bc4c2759` | 304 | 1.1 / 5.9 | 13 | 57.5 / 133.6 | 1.0 | 0.8 | 0.1 | 615 MHz, 52.5 °C |
| app 2 | `b1bc4c2759` | 226 | 1.5 / 4.8 | 5 | 62.3 / 218.8 | 1.0 | 0.9 | 0.1 | 348 MHz, 49.6 °C |
| app 3 | `b1bc4c2759` | 238 | 1.3 / 4.7 | 8 | 60.4 / 219.6 | 0.9 | 0.8 | 0.1 | 295 MHz, 49.2 °C |

### The iPhone

The iPhone reads no GPU utilization, clock or temperature.

| run | build | frames | tick p50 / p95, ms | ticks over budget | GPU p50 / p95, ms | scene, ms | demand, ms | companion, ms |
|---|---|---|---|---|---|---|---|---|
| Safari 1 | `a017105040` | 283 | 2.0 / 4.0 | 0 | 50.1 / 91.6 | 1.7 | 0.6 | 0.1 |
| Safari 2 | `a017105040` | 287 | 2.0 / 4.0 | 0 | 50.2 / 92.9 | 1.6 | 0.7 | 0.1 |
| Safari 3 | did not finish | | | | | | | |
| app 1 | `11ce19d922` | 379 | 0.7 / 1.7 | 0 | 48.9 / 88.5 | 0.4 | 0.4 | 0.0 |
| app 2 | `11ce19d922` | 379 | 0.7 / 1.7 | 0 | 49.6 / 88.3 | 0.4 | 0.4 | 0.0 |
| app 3 | `11ce19d922` | 370 | 0.8 / 1.7 | 0 | 50.5 / 89.8 | 0.4 | 0.4 | 0.0 |

Safari 3 was taken four times and lost each time the same way: its progress stopped
between 25 and 57 s with frames still being drawn, safaridriver's session gone and
nothing said. The phone's auto-lock at a minute does this, as the runs of
[2026-11-16](../20261116-the-deployed-builds-pace-measured/README.md) found.

### The Mac

Nothing else ran on the Mac. `first GPU` is the whole GPU's utilization at a run's first
capture, whoever kept it busy; Safari was quit and left 30 s before each of its runs.

| run | build | frames | tick p50 / p95, ms | GPU p50 / p95, ms | scene, ms | companion, ms | first GPU |
|---|---|---|---|---|---|---|---|
| Chrome 1 | `11ce19d922` | 893 | 2.0 / 2.5 | 20.8 / 29.2 | 1.4 | 0.0 | 59% |
| Chrome 2 | `11ce19d922` | 886 | 2.0 / 2.4 | 20.8 / 29.4 | 1.4 | 0.0 | 57% |
| Chrome 3 | `11ce19d922` | 861 | 2.0 / 2.5 | 21.0 / 30.1 | 1.4 | 0.0 | 52% |
| Safari 1 | `a017105040` | 618 | 2.0 / 2.0 | 21.0 / 31.0 | 1.4 | 0.0 | 67% |
| Safari 2 | `a017105040` | 832 | 2.0 / 3.0 | 15.9 / 25.7 | 1.3 | 0.0 | 59% |
| Safari 3 | `a017105040` | 834 | 2.0 / 3.0 | 16.0 / 25.4 | 1.4 | 0.0 | 65% |

Safari 1 drew 7.9 Mpx where the two after it drew 2.0: its window opened on the built-in
screen, at two device pixels a CSS pixel, and theirs on a screen beside it at one. It is
another picture and is void; the two after it agree with each other and with the watch's
runs.

Two earlier sets on the Mac are not kept. In the first the user was working on the Mac:
two runs drew six frames in ten of the others', and one Safari run was
[the Safari stall](../../gpu/experiment/20261111-the-safari-stall/README.md), the GPU at
100% from its 43 s capture with the page's CPU at 2% and its GPU process's at 0%, the
core stopping at 100.6 s. In the second an agent was compiling on the Mac while Safari
ran, and Safari's frames fell from 611 to 465 over three runs with its tick's p50 rising
from 2 to 4 ms. Read in the session by `ioreg` and `ps`, and in no run's log: 15 s after
a run the GPU read 70 to 92% where it had read 27% before the first, and no WebKit
process was left exiting.

## What came of it

- The commit holds the phone's tick within the gate of the tree before the day, and the
  companion step under it.
- The scene step's 0.3 to 0.4 ms on the phone is the form's: it is not tuned away.
- On the Mac, with the machine to itself and Safari rested between runs, two Safari
  runs one after another agree. A run beside other work on the Mac is void: the
  page's tick rises with the machine's CPU, and the GPU's figures with whoever else
  draws.
- The user's reading of the Mac's Safari: its WebGPU leaks, a run's resources not let go
  as it ends, and the GPU's use piles up over runs until it stands at 100%. The stall
  and the GPU staying busy after a run are with it; the rested set is not against it.

## Not established

- The Mac's Safari on the commit against the tree before.
- The Android phone's Chrome and app, the iPhone's app and the Mac's Chrome on the
  commit's last round: they ran the build before it.
