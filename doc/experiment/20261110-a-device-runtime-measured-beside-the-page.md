# A device's runtime, measured beside the page

**2026-11-10.** The device's runtime off the JavaScript thread (`b7cd54d228`,
`a7e2daa22d`, `b5f4cd06ed`), against the page, and what the report had to be
corrected to say so.

## Tried

The runtime moved to a Rust thread of its own, touches reached the core without
JavaScript, and tile bodies were fetched by the operating system's network and
handed to the runtime. Then a device's frame was made to wait while the runtime
holds its stream (`3005f0a7d5`), as a page's never starts while it would have to
be put off.

## How it was measured

Release builds, `resolution: fixed`, one 20 s `perfDrive` per line, one run
each. System traces beside `s23-app` and the second Chrome run. An audit by
reading the code and comparing `kinds` with the system trace's presents:
`QueuePresentKHR` 269 against `cast` 270 on the S23's app.

## What came of it

| run | AP °C | drawn | Mpx | GPU p50 / p95 ms | GPU p50 per Mpx |
| --- | --- | --- | --- | --- | --- |
| S23 app, before `3005f0a7d5` | 36.0 to 46.3 | 270 | 2.53 | 65.9 / 140.2 | 26.1 |
| S23 app, `3005f0a7d5` | 28.9 to 39.9 | 287 | 2.53 | 61.8 / 135.5 | 24.4 |
| S23 Chrome | 36.7 to 48.2 | 314 | 2.13 | 48.2 / 139.2 | 22.7 |
| S23 Chrome, traced | 32.3 to 43.3 | 306 | 2.13 | 49.9 / 145.9 | 23.5 |
| iPhone app, before `3005f0a7d5` | not read | 360 | 2.74 | 51.6 / 95.5 | 18.8 |
| iPhone app, `3005f0a7d5` | not read | 364 | 2.74 | 50.4 / 91.2 | 18.4 |
| Mac Safari, at 30 Hz | not read | 555 | 7.90 | 22.2 / 32.0 | 2.8 |

- `fixed` does not give the app and Chrome the same pixels: Chrome's canvas is
  1080 by 1969 and the app's 1080 by 2340. Over the window the app drew 682 Mpx
  before `3005f0a7d5` and 725 after, Chrome 651 and 668.
- Before `3005f0a7d5` the device's report counted turns that drew nothing
  (`stream_busy`): `fps` 36.7 for 13.5 frames drawn a second, `interval` p50 17.3
  ms for a drawn gap of 64.7. After it, `fps` is 14.4, `interval` p50 66.6 ms, and
  the report's frames are the drawn ones on both platforms.
- In the first S23 app run's window the GPU clock averaged 604 MHz; in Chrome's
  it held about 700 MHz. The second app run has no system trace.
- JavaScript thread during drags, S23, 28 s: 1.5 to 2.2 s before the tile bodies
  moved and 1.95 s after, one run. The file's open still starts there.

## Not measured

A touch to the screen, the iPhone's Safari, and the app against the page under
touches.

## Kept

All of it. The tile fetch keeps each platform's HTTP cache: OkHttp's on Android
(`0441f85e28`), the system's on iOS.
