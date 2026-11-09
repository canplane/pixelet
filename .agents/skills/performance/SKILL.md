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
- One script a vendor, each one run: `script/perf/<vendor>.py <target> <dir>`.
  `chrome`, `safari`, `android-chrome` and `ios-safari` take a URL,
  `android-app` an APK, `ios-app` an app with `IOS_DEVICE` set to its devicectl
  id. An app run installs afresh, which removes the app's data; `android-chrome`
  clears the site's data and turns the cache off in its own tab.
- Every run writes the same three files into `<dir>`, and all three are read:
  `values.json` (`build`, `load`, `gpu`, `tick`) for the comparison, `report.txt`
  for where the time went, `trace.json` at ui.perfetto.dev for a frame on the
  timeline. `report.txt`'s fps counts the frames put off as `stream_busy`; the
  frames drawn are in its `kinds`.
- What the app's three files cannot see is a device's system trace, taken beside
  a run: `adb shell perfetto --background -o /data/misc/perfetto-traces/<name> -t
  <n>s -b 256mb sched freq idle am wm gfx view input hal binder_driver -a
  me.plei.pixelet`, pulled and queried with Perfetto's `trace_processor`. Read
  `QueuePresentKHR` for the present's wait and the frames a second, `postAndWait`
  and RenderThread's `dequeueBuffer` for the main thread's stalls, and the threads'
  run time for the CPU.
- The drive goes through the core's input and proves nothing of a touch. A touch
  is measured on a build without `PXL_PERF_DRIVE`, by `adb shell input swipe` under
  a system trace, and read as `QueuePresentKHR` a second and `topTouchMove` a second.
- A phone slows as it heats. Each figure on a device says its temperature (`adb
  shell dumpsys thermalservice`, `AP`), and runs are compared only from the same
  temperature, the phone rested between them.
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
- Each experiment gets a journal entry under `doc/experiment/`: tried, came of
  it, how we know, kept or dropped.

Report in three lines: the structure kept; the cost removed and what was added;
the effect checked and the range not checked.
