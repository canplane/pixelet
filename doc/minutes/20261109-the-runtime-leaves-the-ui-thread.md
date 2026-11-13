# The runtime leaves the UI's thread

**2026-11-09** — recorded with the user. Main was at `bda0cb51d8` when this was
written. Where a device's app is going, and why, told as one account of the page
and the device together. It replaces the native column of the table in
`20261106-the-runtime-is-glue.md`, where the runtime sits on the JavaScript
thread. Who decided is said where it was the user; the rest was judged by the
agent working on it against the code. Of what follows only the turn begun at the
display's frame is built.

## Three parts, and what can hold them up

A program on xpute has three parts. The **core** is its compiled code. It owns one
memory and runs only inside a turn, a single call that takes a deadline and answers
when it wants the next. The **runtime** gives the core that time: it decides when a
turn runs, writes what the core is sent into the submission ring, reads what the
core said from the completion ring, and turns all of that into the platform's
calls, the network's, the files', the text engine's and the GPU's. The **UI** is
React, and it draws what the core reports and sends what a finger does.

A thread is only a place where one thing happens at a time, and a phone has eight
cores to run them on. A thread that sleeps costs next to nothing, so the number of
threads decides nothing. What decides is who waits for whom. Three things in this
program wait for a long time, each for its own reason: React, which holds its
thread through a commit; the GPU, whose present waits for the frame before it; and
the network. The design question is which of the three parts stands behind which
wait, and the answer the user holds to is that the UI and the runtime must not
hold each other up.

## The page, and what the browser hides

On a page the UI runs on the main thread and the runtime and the core share a
worker, taking turns on it. The two are in one worker because of memory, not speed:
the main thread cannot reach the core's memory without a `SharedArrayBuffer`, which
the page does not use, so the runtime has to sit where the memory is. The runtime
is in TypeScript because the platform's calls exist only in JavaScript there.

It looks like two threads, and it is many. A WebGPU call hands its commands to the
browser's GPU process and returns at once, and the wait for the frame before
happens in there. `fetch` runs in the browser's network process. Text is measured
by the browser's engine, and the compositor shows the frame. A worker is a whole
environment the browser hands over: a JavaScript engine, an event loop, timers,
`fetch` and WebGPU, all of it free. None of the three long waits ever lands on the
worker's thread, and none lands on the UI's.

## The device, where nothing is hidden

A device has none of that. React Native gives a main thread for the native views,
touches and the display link, and one JavaScript thread for React. Hermes on that
thread is an engine with no JIT, and the event loop, the timers, `fetch` and the
native modules around it are React Native's, tied to that one runtime. Whatever the
browser did behind the page is now the program's own to place.

So the threads a device shows are the browser's, come out into view. The **GPU's
thread** exists because wgpu's present stops its caller: the stream is run and the
frame is handed to the system, and `QueuePresentKHR` waits under `queueBuffer` for
the frame before, on the S23 for as long as 397 ms. The **core's thread** exists
because the core's global state is sound on one thread only, and so that a turn
runs whatever React is doing.

The runtime is the part still in the wrong place. `start.native.ts` boots it where
it is called, so its centre sits on the JavaScript thread beside React: it writes
the ring, reads the ring, serves the files and answers the core's calls. The two
hold each other up in both directions. A busy React delays the runtime, and a text
call stops the core outright until the JavaScript thread answers. The runtime's own
work takes time from React as well, which is the direction a page never has to
think about, since a page's UI thread holds no runtime. The user's conclusion is
that a device's UI thread holds none either.

## Where the runtime goes

It goes to a thread of its own, written in Rust. The page keeps its TypeScript. The
user allowed that split: the runtime is the platform's glue, so each platform has
its own, and one policy written twice, as frame pacing now is, is no fault. It is
not done because the web does it. Each place was weighed by what stops there.

The runtime cannot share the **GPU's thread**, because it must still answer while a
present waits; otherwise a touch and a file's completion wait for the display.
Keeping the present alone apart and running the stream on the runtime's thread
leaves as many threads and spends the runtime's time on the stream.

It need not share the **core's thread**. The page's reason for that does not hold
here: a device's threads share the core's memory freely, and the rings are made for
two threads with no lock, so the runtime's work runs beside a turn rather than out
of it. Sharing the thread would gain only a text call with no crossing, and that is
had anyway by answering the call in Rust on the core's own thread, as a function.

A **second Hermes** on its own thread would have kept the TypeScript. That is
building by hand the worker the browser gives: `XPUTE`, `PLATFORM`, `expo-sqlite`
and `fetch` would each have to be attached to a second runtime, and whether Expo's
modules attach at all was not checked. It would also run the runtime with no JIT
and keep the text call's crossing.

Keeping the runtime on the **JavaScript thread** and moving to Rust only what must
not wait looks at one direction alone, what stops when JavaScript is busy. It
remains the order of the work, and it is not where the work ends.

```text
the page                                the device
  main thread   React                     main thread       views, display link, touches
  worker        runtime (TS) + core       JavaScript thread React
  the browser   GPU, network, text        runtime's thread  runtime (Rust)
                                          core's thread     core, its text calls answered there
                                          GPU's thread      the stream, the present
```

The UI speaks to the runtime through a port on both.

## The order of the work

The core's thread already begins its turns at the display's frames (`8013c4bec7`).
While the JavaScript thread alone writes the submission ring, that turn's frame has
to reach the core outside the rings (`xpute_core::sched::frame`). Next the text calls
are answered in Rust on the core's thread, which removes the one place the core
waits for JavaScript. Then the runtime's thread is made, with the rings, the
scheduler and the file server, and the UI behind a port. It becomes the ring's one
writer, so a turn's grant goes back on the ring as a frame and that outside path
goes.

## What this does not show

The choice was made from the code and from how the platforms work. No build of it
exists, so its effect is not measured. On the S23 during drags the JavaScript thread
ran 2.3 s of 28 s, so what it promises is that React and the runtime stop holding
each other up, not more frames drawn.
