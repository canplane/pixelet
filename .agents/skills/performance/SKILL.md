---
name: performance
description: How a performance problem in pixelet is analyzed and a change to it judged — the contracts a change keeps, the cause found in the code, measurement only to confirm, and a three-line report. Use for any profiling, performance analysis or optimization, in the renderer, the core, the UI or the tools.
---

# Performance

Make the program faster while keeping every condition the design met.

- Name the contracts the change touches first; changing one is a design change,
  proposed on its own.
- Judge whether a part exists by what the frame must produce, from the code. A
  measurement does not put a part in or take it out.
- Find the cause in the code: predict from the structure what it costs and why.
  A costly function is where the time went, not proof its body is wrong.
- One scenario on every platform, the app's alone: `perf/measure.ts` holds where
  it stands, how long a load settles and what it reads; `perf/drive.ts` holds
  the settings, the path and the window. A script holds none of it.
- A measured build takes `PXL_PERF_DRIVE=1` where it is built and served, and is
  dev or release, always said with its figures and never mixed:
  - dev: a page's is `PXL_PERF_DRIVE=1 make dev`; a device's is the device-build
    skill's Debug build on `PXL_PERF_DRIVE=1 make dev-native` (`METRO_PORT` when
    that Metro is not on 8081).
  - release: a page's is `PXL_PERF_DRIVE=1 make preview`; a device's is the
    device-build skill's Release build with the variable set.

  When the measuring ends, the device takes a Release built without it.
- One script a vendor, each one run: `script/perf/<vendor>.py <target> <dir>`.
  `chrome`, `safari`, `android-chrome` and `ios-safari` take a URL,
  `android-app` an APK, `ios-app` an app with `IOS_DEVICE` set to its devicectl
  id. `android-app` installs afresh, which removes the app's data; `ios-app`
  installs over what is there, which keeps it; `android-chrome` clears the
  site's data and turns the cache off in its own tab.
- A run starts its browser or app afresh and ends it, so no run shares a
  process or a GPU's with one before; where it cannot start afresh it starts
  none and says what runs. Close Safari before a run. `chrome` opens a fresh
  profile's Chrome of its own, `android-chrome` stops the phone's Chrome before
  it starts it, the apps are launched afresh, and whether the iPhone's Safari
  runs is not read.
- A run measures a page or an app shown from the core's start to its result
  (`perf/measure.ts`). A window wholly over it, the window minimized or on
  another Space, another tab in front, or the app not active hides it,
  and the run ends with `the page is hidden`; a window beside it or over part
  of it does not. Nothing raises a window: a run that needs the screen waits
  until the user can leave it shown.
- Every run writes the same into `<dir>`, and all of it is read: `values.json`
  (`build`, `longest`, `gpu`, `tick`) for the comparison, `report.txt` for where
  the time went, `trace.json` at ui.perfetto.dev for a frame on the timeline;
  and, however the run ends, `said.txt` and `shots/`. `said.txt` holds, each
  after the run's seconds, every line the app said but the result, the runtime's
  own among them (a core's stop, a device lost, an uncaptured error, and in a
  measured build a GPU record by its op's number, a GPU run, a GPU
  completion's landing or a core's entry slower than perf.json's
  `LIMIT.SLOW_MS`), then why
  the run ended. The app's progress is a line a second from the core's start,
  whatever the core answers (`PXLRUN`: whether it is shown, the boot's state,
  how long the core's read has gone unanswered, then perf.json NOW `draw` as
  last answered: its frames, the granted frames that did not run by
  why, the device's generation, STATUS, uncaptured errors and unfinished
  frames). Each capture, every `CAPTURE_S` (script/perf/measure.py), is a
  `shot` of the screen into `shots/` and the device's `state`: first how busy
  the device's GPU is, whoever keeps it busy (`gpu_util`, `None` on an
  iPhone, which reads none), then the phone's GPU clock and AP temperature;
  the Mac Safari's GPU process's aborted command buffers, and Chrome's log of
  its GPU process, each then with the CPU its page, its GPU process and the
  Metal compiler took. A
  reach that has not started within `START_S` ends the run, saying so.
- What a run needs to explain itself (its progress, the screen, the device's
  state) comes through the one path every platform shares; no run adds a
  script, a log or a capture of its own. A symptom seen on the screen is read
  off the run's captures beside its numbers.
- What the app's files cannot see is a device's system trace, taken beside
  a run: `adb shell perfetto --background -o /data/misc/perfetto-traces/<name> -t
  <n>s -b 256mb sched freq idle am wm gfx view input hal binder_driver -a
  me.plei.pixelet`, pulled and queried with Perfetto's `trace_processor`. Read
  `QueuePresentKHR` for the present's wait and the frames a second, `postAndWait`
  and RenderThread's `dequeueBuffer` for the main thread's stalls, and the threads'
  run time for the CPU.
- The drive goes through the core's input and proves nothing of a touch. A touch
  is measured on a build without `PXL_PERF_DRIVE`, by `adb shell input swipe` under
  a system trace, and read as `QueuePresentKHR` a second and `topTouchMove` a second.
- A phone slows as it heats. Each figure on a device says its temperature, from
  its run's `state` lines, and a run starts as the phone stands.
- The iPhone's memory is `memory-physical-footprint` in `sysmon-process`, from
  `xcrun xctrace record --device <id> --template 'Activity Monitor' --all-processes
  --time-limit <n>s` started after the launch, and `Jetsam` records from `xcrun
  devicectl device info files --domain-type systemCrashLogs`.
- A task measures. `script/perf/gate.py <vendor> <base> <change> [rounds]` runs
  only when the user orders a comparison, on the two builds they name, the
  deployed build included. It only assembles: it runs the vendor's script on them
  by turns, keeps each run under `tmp/perf/` and compares their `values.json`.
- The devices: the S23's Chrome (Tint, Vulkan, a weak mobile GPU) for
  performance, hot; Safari on the Mac (WebKit's compiler, Metal) for the iPhone.
  Mac Chrome only shows a page running. Mobile web is the minimum; no polyfill
  or fallback for what time will fix.
- Each experiment gets a journal entry under `doc/experiment/`, or
  `doc/gpu/experiment/` for the GPU's: tried, came of it, how we know, kept or
  dropped.

Report in three lines: the structure kept; the cost removed and what was added;
the effect checked and the range not checked.
