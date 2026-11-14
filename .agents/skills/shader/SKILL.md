---
name: shader
description: Change the WGSL or what a shader is assembled from, and check it where the tests cannot — what a shader keeps clear of, the order of the checks, and how to run the shader probe in Safari and on a phone. Use before and after changing a shader, `frame/shader/part.rs` or the words shaders share. What a frame costs is the performance skill's.
---

# Shader

Before writing, read:

- `doc/gpu/lesson/shaders-phones-take.md`: what a shader keeps clear of for the
  phones' browsers, and what a host and a shader may share.
- `doc/gpu/lesson/naga-on-metal.md`: what the native apps' translator gets wrong.
- `doc/gpu/lesson/cpu-and-gpu-agree.md`, for a value the CPU and the GPU both
  decide.

After writing, in this order:

1. `./script/diff-check.py`.
2. The shader probe in Safari on the Mac.
3. The page, for what the change draws (`PXL_PREVIEW_PORT=<port> make preview`).
4. The S23, or name it as not run.

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
- Keep the page shown until it ends: hidden, as the performance skill says a
  page is hidden, it stops compiling, with no error and no line printed.
- It has ended at `[probe] done`, and only there. A silence with the Mac's
  `MTLCompilerService` idle is a stopped page, not a slow module: show the page
  again and probe the rest with `?only=`.
- A module's or a pipeline's line without `ok`, and `[probe] device lost`, is a
  refusal.
- A module's name starts with its number (`70-queue_walk`). `?only=<prefix>,…`
  probes the names that start so. `?bisect=<name>` finds the line a device
  refuses, `?from=` and `?to=` narrowing it. `?src=<end of a path>` probes a
  shader recorded under an experiment of `doc/`.
- Stop the server when it has ended.

## The S23

With the probe served on port 5173 under script/perf/android.py, over USB, `adb forward tcp:9333
localabstract:chrome_devtools_remote` and `adb reverse tcp:5173 tcp:5173`, then
`doc/gpu/experiment/20261103-the-shaders-by-hand/tool/cdp.js` compiles a WGSL file
there from the Mac.
