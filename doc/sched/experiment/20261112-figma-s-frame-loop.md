# Figma's frame loop, as its editor runs

**2026-11-12.** Who enters whom, the JavaScript or the C++, in Figma's design
editor. This is one session's observation of one file in the web editor, on that
day. It is not evidence for Figma's other editors, its other paths, or its
versions before or after.

## How it was read

Chrome on a Mac, through the Claude in Chrome extension, in one of the user's own
design files. Nothing in the file was edited. The canvas was scrolled to make
frames.

- Wrappers on `GPUQueue.prototype.submit` and
  `GPUCanvasContext.prototype.getCurrentTexture` saved the call stack, with
  `Error.stackTraceLimit` raised, on the frames the scroll drew.
- The same wrapper counted `getCurrentTexture` over two seconds with the view
  held still.
- The Emscripten glue (`Fig.fullscreenURLs`, `compiled_wasm.js`) and the app's
  bundles (`figma_app__react_profile`, chunk `39902`) were fetched as the page
  holds them and searched for the functions on the stack.
- The glue was read again outside the page, fetched and decompressed with Node, and
  its 2,404,584 characters match the page's. That copy was searched for a stack
  suspension.

The first wrapper on `window.requestAnimationFrame` kept twenty records, and a
frame-measuring helper's calls filled them. A later wrapper that kept only the
frame loop's callback caught it (below).

## What came of it

**The stack of a drawn frame**, outermost last:

```text
GPUQueue.submit / getCurrentTexture
  _wgpuQueueSubmit / _wgpuSurfaceGetCurrentTexture   the glue's WebGPU import
  wasm-function[9330] … [48057]                      C++
  _wrapCppCall, _FullscreenUi.onFrame                the glue's export
  onFrame                                            the app (chunk 39902)
  the requestAnimationFrame callback                 the app (figma_app)
```

The canvas has a WebGPU context, on the main thread. The page's resources list a
library-search worker and no render worker, though `Fig.renderBundleURLs` names a
render-worker build of the wasm.

**The loop is the app's.** `figma_app` holds one request function, `H()`. Once a
handler is registered, `H()` calls it, and the handler puts in one
`requestAnimationFrame` unless one is already pending. In the callback, in this
order:

- `fullscreen.onFrame()`, which enters the C++ through `_FullscreenUi.onFrame`,
  with no arguments
- the other views' `onFrame`s
- the redux frame handler
- the frame listeners

Then the callback runs what was queued for after the frame.

Each request is for one frame. With the view held still, `getCurrentTexture` was
called 0 times in two seconds, and in the second session the frame loop ran twice
in three seconds.

**Who calls `H()`:**

- the C++ (`Async.requestAnimationFrame`)
- a redux dispatch
- a viewport change
- a deferred subscription's change

**What the C++ imports for time.** These are the seven functions of the `Async`
binding. The app serves them from one object:

| import | the app's body |
| --- | --- |
| `requestAnimationFrame()` | `H()` |
| `postUserBlockingTaskHelper`, `postUserVisibleTaskHelper`, `postBackgroundTaskHelper` | `scheduler.postTask` at that priority, with a `TaskController`; the promise goes back to the C++ |
| `cancelTask` | aborts the task's controller, or clears its timeout |
| `setTimeout(id, ms)` | `setTimeout`, then the C++'s `timeoutCallback(id)` |
| `clearTimeout` | `clearTimeout` |

`Async` is one of many binding objects the glue takes from the app by name, such
as `AutoLayoutBindings` and `AssetMirrorBindings`. They are the C++'s imports, as
WebGPU's functions are. The stacks caught only WebGPU's calls. The glue has no
`emscripten_set_main_loop`, and nothing that suspends a wasm stack: none of
`Asyncify`, `asyncify_start_unwind`, `asyncify_stop_rewind`, `handleSleep` or
`emscripten_sleep` for Asyncify, and neither `WebAssembly.Suspending` nor
`WebAssembly.promising` for JSPI.

## How long the C++ holds the thread

A second session the same day, in the same Chrome. Two of the user's files were
opened in turn from the files list in one tab: the first file again, then a
larger one. The display's frames came 8.3 ms apart (120 Hz).

- A wrapper on `window.requestAnimationFrame` kept only the frame loop's callback,
  whose source names `setRanFullscreenEventLoop`. It timed each callback, took its
  end from the frame's timestamp, and counted the `performance.now` calls made
  inside it.
- A wrapper on `Scheduler.prototype.postTask` recorded each task's delay and
  whether a wasm function posted it. For each promise the C++ posted, it also
  timed the callbacks attached to it with `then`.
- A `long-animation-frame` observer recorded each frame over 50 ms and the scripts
  in it.

**The frame loop's callback**, which holds `onFrame`:

| while | frames | ms each |
| --- | --- | --- |
| scrolling and zooming | 14 | 0.4 to 4.5 |
| the larger file loading, over about 25 s | 111 | 3 at p50, 4.6 at p90, 24 at most |
| the first file loading, in one unbroken run over 23 s | 1,894 | 6 to 7 or 12 to 13; their starts 13.1 apart at the median |

- While the larger file loaded, a callback ended 2.2 to 5.3 ms after the frame's
  timestamp (p10 to p90), so not at one fixed time.
- The first file's run was taken with a probe that captured a stack on every clock
  read, and that costs time. Whether its two levels are the C++'s or the probe's
  is not known.
- In a frame the C++ read `performance.now` hundreds to thousands of times, through
  `_Support_currentTimestampFromEmscripten` and `trackUserActionTimings`. Whether
  any of those reads decides when it returns was not determined.

**The tasks the C++ posted.** While the larger file loaded, the C++ posted about
385 tasks. 376 of them had a 33 ms delay; the rest had 0, 200, about 450, about
960 or 5,000 ms. A posted task's own body is the app's: it drops its handle and
returns `true`. The callbacks attached to those promises ran 1,087 times: 0.3 ms
at p50, 0.5 at p90, 5.7 at most. Whether they enter the C++ was not seen.

**What ran over 50 ms** while the larger file loaded:

| task | ms |
| --- | --- |
| the multiplayer `WebSocket`'s `onmessage` | 1,172 |
| a `MessagePort`'s `onmessage` | up to 215 |
| a `requestAnimationFrame` callback the frame-loop wrapper did not record | up to 159 |
| an `XMLHttpRequest`'s `onload` | up to 136 |

The `WebSocket`'s handler logs the message, then passes its bytes to
`FullscreenWebSocketTsApi.receive`, a C++ export. That task held the thread for
more than a second. What the message carried, and where in the task the time
went, was not seen.

**Where the C++ is entered.** The glue wraps 5,092 C++ functions for the app to
call (`_wrapCppFunction`). Each goes through `_wrapCppCall`, which
`preventEnteringCpp()` can refuse. The glue also takes 134 binding objects from
the app. Any task that calls one of those functions enters the C++:

- the frame loop;
- the `WebSocket`;
- a timer, through `AsyncTimeoutCallback.timeoutCallback`.

## What was not seen

- How long the C++ itself ran inside the callbacks and tasks timed, which bound
  it from above, and whether it holds itself to a budget in a frame.
- How the C++ goes on after a `postTask`'s promise settles.
- When the C++ calls `Async.requestAnimationFrame`, and whether inside `onFrame`.
- How input reaches the C++, and whether it is called inside the event or put
  off to the frame.
- Whether the render-worker build runs on another path.
