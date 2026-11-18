---
name: shader
description: Change the WGSL or what a shader is assembled from, and check it where the tests cannot — what a shader keeps clear of, the order of the checks, and how to run the shader probe in Safari and on a phone. Use before and after changing a shader, `frame/shader/part.rs` or the words shaders share. What a frame costs is the performance skill's.
---

# Shader

## What a shader keeps clear of

The build refuses in every assembled module what it can see of this
(render build/check.rs `refuse_faults`): a struct that holds a struct, a local
that is a struct or holds an array, a function that returns what holds an
array, an array of vectors read at a place found as the shader runs,
`i32::MIN` written or folded, `atomicCompareExchangeWeak`, and a
`var<uniform>` or `var<storage>` with room inside it. What no check sees:

- A claim is an exchange that puts back what was not its own. A take from a
  counter is an add, given back with a min where it did not fit, never with a
  subtract while others take.
- A choice between two implementations that answer in tuples or structs is
  made when the shader is assembled (`frame/shader/part.rs` `PARTS`), never as
  it runs.
- What a `var<uniform>` or `var<storage>` holds is bound as the words it is on
  the wire and read a field at a time by a function of its own. The whole
  struct is never built back from the words.
- A loop ends by a `for` condition or a `break` in its body: no `continuing`
  block and no `break if`.
- A module may be refused for its size or its load alone, with no shape at
  fault.
- A rule of the world's state is exact: in integers, or in floats that stay
  exact, its bits counted first, with no division in a decision. A value
  several pipelines decide alike at an edge is exact too. A reciprocal or a
  root a frame holds constant is rounded once on the host and handed over in
  a word. A drawing rule goes through one function, and its test names the
  rounding it allows and counts the cases.

The case behind each is under `doc/gpu/lesson/`: `shaders-phones-take.md` for
the phones' browsers and what a host and a shader share, `naga-on-metal.md`
for the native apps' translator, `cpu-and-gpu-agree.md` for a value the CPU
and the GPU both decide.

After writing, in this order:

1. `./script/diff-check.py`.
2. The shader probe in Safari on the Mac.
3. The page, for what the change draws (`PXL_PREVIEW_PORT=<port> make preview`).
4. An Android phone, or name it as not run.

## The probe

It makes every module as the renderer assembles it, with every pipeline the
renderer makes over it, on the device it is opened on.

Ask the user before taking the browser (AGENTS.md), and to bring the page
forward.

```sh
cd ts/tool/shader
CARGO_TARGET_DIR=<root>/rust/target/session/<id> ../../../script/perf/mac.py pnpm exec vite --host --port <port> --strictPort
open -g -a Safari http://127.0.0.1:<port>/probe.html
```

- Read the result in the server's output or `ts/tool/shader/.dev-console.log`:
  a line for each module and each pipeline, `[probe] <name> <entry>: ok (<ms> ms)`.
- Keep the page shown until it ends: hidden, as the measure skill says a
  page is hidden, it stops compiling, with no error and no line printed.
- It has ended at `[probe] done`, and only there. A silence with the Mac's
  `MTLCompilerService` idle is a stopped page, not a slow module: show the page
  again and probe the rest with `?only=`.
- A module's or a pipeline's line without `ok`, and `[probe] device lost`, is a
  refusal.
- A module's name starts with its number (`70-queue_walk.first`).
  `?only=<prefix>,…` probes the names that start so. `?bisect=<name>` finds the
  line a device refuses, `?from=` and `?to=` narrowing it. `?src=<end of a
  path>` probes a shader recorded under an experiment of `doc/`.
- Stop the server when it has ended.

## An Android phone

With the probe served on port 5173 under script/perf/android.py, over USB, `adb forward tcp:9333
localabstract:chrome_devtools_remote` and `adb reverse tcp:5173 tcp:5173`, then
`doc/gpu/experiment/20261103-the-shaders-by-hand/tool/cdp.js` compiles a WGSL file
there from the Mac.
