# What a frame asks of WebGPU

**2026-09-22.** How many calls the shell's WebGPU translator
(`apps/web/src/kernel/render/gpu.ts`) makes in a frame, and how many
JavaScript objects it makes to make them. The renderer is Rust and holds its
world in the module's linear memory, so the only place a frame can make
garbage the collector sees is that translator.

## How

In the page, each WebGPU method the translator calls was wrapped on its
prototype and counted, and the counts divided by the frames
`requestAnimationFrame` saw over the same interval. Nothing in the
repository was changed to measure. The published build at plei.me, desktop
Chrome at 120 Hz: five seconds at rest, 600 frames, and six seconds of two
drags, 720 frames. Then the same count on the dev build with the fix below,
956 frames with one drag.

## What a frame calls

| a frame                            | at rest | dragging |
| ---------------------------------- | ------- | -------- |
| `createBindGroup` / `createBuffer` | 0 / 0   | 0 / 0.01 |
| `createCommandEncoder`, `submit`   | 1 each  | 1 each   |
| `createView` (the canvas)          | 1       | 1        |
| `beginRenderPass`                  | 2       | 2        |
| `writeBuffer`                      | 7       | 7        |
| `setPipeline`                      | 4       | 4.3      |
| `drawIndexed` / `draw`             | 241 / 1 | 241 / 1  |
| `setIndexBuffer`                   | 241     | 241      |
| `setVertexBuffer`                  | 1,167   | 1,161    |
| `setBindGroup`                     | 397     | 400      |
| … with dynamic offsets             | 131     | 132      |

Nothing on the device is made per frame: groups, buffers and pipelines are
made once and kept, and a drag across the map made seven buffers in six
seconds.

## What it showed

`setBindGroup` made garbage on every call. With no offsets it passed a fresh
`[]`; with some it made a `Uint32Array` view and copied it with
`Array.from`. That is some 530 objects a frame, about sixty thousand a second
at 120 Hz — small against a scavenge, but it grows with what is drawn, at 1.66
calls a draw, which is the one thing this design meant not to.

Fixed in c10a01fb: no offsets, no argument; with offsets, WebGPU's
`(data, start, length)` form over one view of the whole memory, made again only
when growth detaches the buffer. Measured after: 389 calls a frame, 261 with
no offsets, 128 through the view, **0** with an array, the offsets read right
(the largest 32,000) and no validation error.

Left as it is: `setVertexBuffer` at 4.8 calls a draw. It makes nothing, but
each is a crossing into JavaScript and a WebGPU validation, and it is most of
what the translator does in a frame. It is the renderer's to change, by
binding a chunk's lanes as fewer buffers, not the translator's.
