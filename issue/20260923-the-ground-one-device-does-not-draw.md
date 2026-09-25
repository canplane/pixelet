# The ground one device does not draw

A Galaxy S23 drew the decor, the models and the sky, and no terrain at all.
An iPhone 13 mini drew everything. So did a Mac, in both Safari and Chrome,
and so did an iPad. The ground had been missing on that one device since the
bake moved onto the arena, and nothing anywhere said so.

It was a struct. Every chunk draw is eight floats the renderer lists and the
shader reads back — where the mesh sits, whether it shows, the cell its boxes
are measured in, the step their heights count, and which of the two box
formats it holds. The shader declared them opening on a three-float vector:

```wgsl
struct Draw { translation: vec3f, visible: f32, opacity: f32, unit: f32, … };
@group(0) @binding(3) var<storage, read> draws: array<Draw>;
```

A `vec3f` is twelve bytes and aligns to sixteen. WGSL puts the float after it
at twelve; this device's backend put it at sixteen. So the first member read
and every member past it took its neighbor's value, and the shader spent
each draw's numbers one slot along from where they were written.

## What the device was

|        |                                                                        |
| ------ | ---------------------------------------------------------------------- |
| failed | Galaxy S23, Chrome 154, Qualcomm Adreno 7xx                            |
| drew   | iPhone 13 mini (Safari 26.1), iPad, Mac (Safari 26.6.1 **and** Chrome) |

Chrome on the Mac drew it, so this was not Dawn against WebKit. The iPhone 13
mini drew it, so this was not a weak mobile GPU. Every device ran the same
build, over the same tunnel, against the same bucket. What the failing device
had to itself was a backend: Metal on all four of the others, Vulkan on it.

## What each field became

The record is written `[x, y, z, visible, opacity, unit, z_step, box_words]`.
Read a word late from `visible` on, and every symptom the screen ever showed
falls out of one table.

| the shader asked for | it was handed           | what that drew                                          |
| -------------------- | ----------------------- | ------------------------------------------------------- |
| `translation`        | itself                  | a cube placed by it alone landed where it should        |
| `visible`            | `opacity`               | both 1, so nothing                                      |
| `opacity`            | `unit`                  | nothing the eye could name                              |
| `unit`               | `z_step`                | boxes measured in the wrong cell, piling where they met |
| `z_step`             | `box_words`, 6.0 or 4.0 | heights several times over — the threads into the sky   |
| `box_words`          | the next draw's `x`     | a coordinate, so the format flipped draw to draw        |

The last row is why the screen looked different every few frames, and why a
probe that reached into a box's own words could come back right: the test
that picks the box format was reading a world coordinate, so a draw took the
leaf's format or the coarse one depending on where its chunk stood.

The other shaders' draw records have the same shape and none of them failed,
because theirs are `var<uniform>`, bound one draw at a time. The chunk
shader's is the only one that is a struct inside a `var<storage>` array.

## Why every instrument said it was fine

The ledger reported the terrain as drawn: `chunk 51 draws / 315k tris`,
`level_tris [L0 315k]`, every cap at zero, no slot wait, no stale chunk, 24
pages resident, 436 tiers built, nothing refused. The fetch chain, the
records, the mounts, the builds and the draw list were all clean — and they
were right. The draws happened. They drew the wrong shape, and a census of
counts is the wrong instrument for that.

It could not have caught a weaker fault either, because **the census counts
the draw list, not the draws** (`gpu/renderer.rs`'s `frame_stat`). A draw the
renderer skips is still in the list it counted. Three places report success no
matter what happens:

- `gpu/arena.rs` creates its six buffers and never looks at whether they were
  made — the id goes into the table either way, and `arena_bytes` counts a
  group by its id, so the panel reads 236 MB whether or not the device has
  any of it;
- `gpu/renderer.rs`'s `draw_meshes` skips a draw whose buffer is missing with
  a bare `continue`;
- `frame_stat` counts what was listed.

None of that was the cause, and two of the three are closed below. The first
is not: the shell already asks an out-of-memory scope and says when a buffer
is refused, but the kernel never learns, because the answer comes back on a
promise and the id has long since gone into the table.

## How the device was read at all

Neither device can be attached to: Safari wants a Mac on a cable, Chrome on
Android wants `chrome://inspect` on a desktop. So the page says its console
down the socket it was served over — `feature/dev/remote-console.ts` forwards
`console.*` and both unhandled-error events to `/__console`, and
`vite.config.ts` prints them and appends them to `apps/web/.dev-console.log`.
Dev only, and gone from every built bundle.

WebGPU needs a secure context, and a LAN address over plain http is not one,
so the device reached the dev server through a tunnel
(`cloudflared tunnel --url http://localhost:5173`, with `.trycloudflare.com`
in `server.allowedHosts`).

That channel is the reason any of this was possible, and it stayed: it caught
a regression in this very fix, on two devices, within ten seconds of a
reload.

## The twenty that were not why

Each line is a hypothesis and the measurement that ended it.

|                                                  |                                                                                                                                                                                                                                                                                                                      |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| zstd, its level, its window                      | Frames with **no fetch in flight** are as slow as frames with one (91.0 ms against 94.0, n=292/58). Decompression is single-threaded and level-independent, the frame header says the window is 0.50 MiB against Chrome's 8 MiB ceiling, and iOS 26.6.1 decodes zstd anyway.                                         |
| the device running out of GPU memory             | All six arena buffers are created, no `out-of-memory` scope fires, nothing reaches `onuncapturederror`.                                                                                                                                                                                                              |
| the whole arena held at once                     | The six buffers the renderer holds together — five of 40 MiB and one of 36 — all made, all written at their first, middle and last slot, all read back through the vertex fetch that reads them. Then 40 MiB at a time on top, until refused: still granting at **1196 MiB** held.                                   |
| a limit the device does not have                 | The adapter reports **exactly** what the Mac reports: `maxStorageBufferBindingSize=134217728`, `maxBufferSize=268435456`, `maxStorageBuffersInVertexStage=8`, alignments 256, `maxBindGroups=4`, and `core-features-and-limits` — so not compatibility mode either. The arena binds 40 MiB of the 128 it is allowed. |
| the buffer being too large                       | Cutting `SLOTS` from 576 to 48 takes the arena from 236 MiB to 19.7. The ground stays missing.                                                                                                                                                                                                                       |
| the pipeline, the depth state, the rasterizer    | A vertex shader that emits one fixed triangle and reads nothing paints the screen on the failing device.                                                                                                                                                                                                             |
| the bind group naming the wrong buffer           | The group the draws bind names buffer 65718; the writes go to buffer 65718.                                                                                                                                                                                                                                          |
| the draw's own arguments                         | Identical in shape to the working devices: one instance, `first_vertex` 0 → 98280 → 196560, vertex counts in the same range.                                                                                                                                                                                         |
| the writes not landing                           | Read back off the device and compared word for word against what was sent: **SAME**, on both, at offsets 0, 65520 and 131040.                                                                                                                                                                                        |
| the write racing the draw                        | The write of a region is logged before the draw that reads it, and a standalone page that writes and draws for 24 frames reads the first region back intact.                                                                                                                                                         |
| `firstVertex` not reaching `vertex_index`        | A standalone page reads the right word at `firstVertex` 98280 and 196560 on the failing device.                                                                                                                                                                                                                      |
| `firstInstance` not reaching `instance_index`    | The same page, at instance indices up to 16391.                                                                                                                                                                                                                                                                      |
| a storage buffer read in the vertex stage        | The same page, at 40 MB and at offset 65520. All seven of its cases pass on the failing device, bit for bit with the others.                                                                                                                                                                                         |
| an instance-stepped vertex attribute             | Both strides, both slots over one buffer, at the offsets the arena binds — up to 41,796,384 — every one of eight columns correct.                                                                                                                                                                                    |
| the draw record's index on `firstVertex`         | A first vertex of 1493 cubes reaches the shader as 1493, and the attributes under it do not move.                                                                                                                                                                                                                    |
| the `Draw` struct read in the vertex stage       | Records 0, 1, 1493 and 4095, every field right — **in a shader of one binding**, which is why it passed. See below.                                                                                                                                                                                                  |
| the sky drawing over it                          | With the sky, the stars and the clouds not drawn at all, the failing device is black.                                                                                                                                                                                                                                |
| the face mask reading as zero                    | Drawing every face regardless of the mask changes nothing.                                                                                                                                                                                                                                                           |
| a module-scope `const` array indexed at run time | Reaching the same corner through a function-local `var` changes nothing.                                                                                                                                                                                                                                             |
| WGSL struct alignment                            | **This one was right, and the wrong buffer was looked at.** What read as nothing is the arena, `array<u32>`, a flat array with no layout to get wrong — and the row was ticked off on that. The record beside it is `array<Draw>`, which is nothing but layout.                                                      |

## Why isolating a mechanism never found it

Three standalone pages were written, each using no project code, each
rendering pixels it read back, so a device with no console could still say
what it saw. Between them they covered the storage read, the instance-stepped
attribute at the arena's own strides and offsets, the draw index on the first
vertex, the six buffers held together, and the `Draw` struct itself. **Every
case passed on the failing device.**

They had to. A layout disagreement is not a property of a mechanism but of a
declaration, and a page that declares the struct in a shader of one binding is
a page whose compiler had no reason to get it wrong. The smallest program that
shows a driver fault is worth what it costs, and these were built at the right
moments and killed what they were built to kill. What none of them could do is
find a fault that exists only in the real thing — and after the fifth one that
passed, the passing was the finding.

What found it was cutting the real shader down on the device. Two cuts.
Replace `unit` with the constant the finer of the two box formats always has,
and the giant boxes that came back said the draw record was lying. Force the
format test, and the ground appeared with its heights still wrong, which named
the half of the record that was lying. Both cuts mean nothing on a correct
device — that format's `unit` is always 1, and its pipeline is already its
own — which is what made them readable. A probe that also changes the answer
says nothing.

Two cautions that still hold about the table above. The color probes — "red
if this, green if that" — sample **one** vertex of thousands of overlapping
full-screen triangles, so they prove "at least one box passed", never "all
did"; only the probes read as a _shape_ (piled against scattered) carried
their weight. And this remains a description of one device: what is recorded
here is that two declarations of the same eight floats behave differently on
it, not a fact about Adreno in general.

## What changed

- **The record is two four-float vectors.** The eight floats are written
  exactly as before; the shader reads them as two sixteen-byte halves, which
  no layout rule can place two ways.
- **The mesh arrives as instance-rate vertex attributes.** This came first and
  was not the fix — the storage pull would have worked once the record did. It
  stays because it is the right shape: a box is one record thirty-six vertices
  share, drawn as a run of consecutive boxes, which is what an instance-rate
  input is for, and the hardware's vertex fetch does the address arithmetic
  the shader was doing by hand.

  ```
  was   set_bind_group(1, arena)
        draw(box_cnt * 36, 1, first_box * 36, draw)
        boxes[vertex / 36 * box_words + n]

  is    set_vertex_buffer(0, arena, first_box * stride)
        draw(36, box_cnt, draw * 36, 0)
        @location(n) the box's words, stepMode "instance"
  ```

  The arena, its slots, its pool and who owns the memory are untouched. This
  is not the pre-arena block layout coming back — that was a different
  question, about where a mesh lives, and it was answered (20260922,
  `904a6721`).
- **The draw record's index rides on the first vertex.** WebGPU fetches an
  instance-stepped element at `first_instance + i`, so a first instance is
  added to the binding's offset rather than read beside it, and cannot also
  carry the record's index. No attribute steps per vertex, so the first vertex
  slides nothing: the draw gives it the index times the cube, and the shader
  divides it back out.
- **The two box formats are two vertex entry points.** They were already two
  pipelines, one per stride, so which format a draw holds is the pipeline's,
  and the float the shader read it back from is gone from the record. A
  pipeline record names its entry point now (`record.VS`, `record.VS_COARSE`),
  and `every_pipeline_matches_its_shader` holds a pipeline's attributes
  against the inputs of the entry point it actually enters.
- **A record's head is one number.** Adding that word broke the frame, because
  three parsers each held their own copy of how many words a record opens
  with and only two were changed. The number is `record.HEAD` in the spec, and
  all three read it.
- **A draw that cannot be issued stops the frame.** Both cases the bare
  `continue` swallowed are faults in the list — an arena group with no buffer
  is one the arena does not configure, and a block handle that will not
  resolve is one already freed — so neither is a frame to get through. The
  census then needs no second counter: every draw listed is a draw issued, so
  reading the list is reading the draws. It also turned up an arm the split
  into two pipelines had left behind, `PIPELINE_CHUNK_COARSE` falling through
  to zero triangles, which is every tier past the leaves.
- **The gap and the step are two numbers.** `MAX_DELTA_S` sat in the strobe,
  clamping what the host measured before anything saw it. How long a gap still
  counts as motion is a question about what the guest simulates, so the
  constant left the runtime: the host reports what passed, the world is
  stepped by at most `MAX_STEP_S`, and `dt_ms` is the interval its own field
  had always claimed it was.

## Beside it, and not fixed here

- **A refused buffer is still silent to the kernel.** The shell knows; the
  arena does not, so `arena_bytes` counts a group whose id was handed out
  rather than one whose buffer was made. The answer arrives on a promise,
  which makes this a question about how a refusal gets back across, not a
  measurement.
- **The iPad's frame rate.** 91 ms a frame at 4.9 Mpx, of which the kernel is
  1 ms and the GPU 17: 73 ms is neither, and it does not move when the draw
  count does. It is outside both spans, so no lane added inside them can see
  it. The world pass is the only span that is not zero — there is no depth
  prepass — and the phone that drew no terrain at all ran at 60.
