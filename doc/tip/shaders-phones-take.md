# Shaders the phones take

**2026-11-03.** What a shader keeps clear of so the phones' browsers compile
it, learned on a Galaxy S23 (Adreno 740, Chrome) and an iPhone 13
mini (Safari 27), with Safari on the Mac failing as the iPhone does. The
evidence, the reduced cases and the tools are
[the shaders on two phones](../experiment/20261103-the-shaders-by-hand/README.md).
These hold whatever language the shaders are written in: they are about the
WGSL a browser receives and what its compiler and the phone's driver do with
it. The shaders were then generated from Rust (rust-gpu, naga, a rewriter);
they are written by hand in WGSL now
(`rust/client/render/src/frame/shader/`), and what the rewriter did for the
phones is a rule the hand WGSL keeps (below).

Desktop Chrome passing proves little here. Chrome on the Mac compiles WGSL
with Tint to Metal for a desktop GPU; Safari compiles it with WebKit's own
compiler; Chrome on Android compiles it with Tint to SPIR-V for the phone's
own Vulkan driver. Each is a different compiler with its own faults.

## Keep clear of

- **`atomicCompareExchangeWeak`.** Safari writes Metal for it that does not
  compile, on the iPhone and the Mac alike. Claim with an exchange and put
  back what was not yours (ask.wgsl `claim`); take from a counter with an
  add and give back with a min where it did not fit (way_tree.wgsl `pop`).
  Never give back with a subtract while others take: it overlaps what they
  took.
- **`i32::MIN` written as a constant, or folded into one.** Safari writes it
  as a bare literal, a C++ `long`, and a `select` of it and an `int` fails:
  on 2026-11-05 `way_tree_cs` and `bound_cs` were refused so, and the page
  stood at its loading. A sentinel or a fold's start takes the end of the
  range its values come from instead (way_rule.wgsl `FACE_NONE`, the head
  word's i16 none; way_tree.wgsl `RUN_LOWEST`; ground_height.wgsl's `lo` and
  `hi`). render build.rs `held_to_faults` refuses a module that holds
  `i32::MIN`.
- **An atomic in a branch of its own.** naga reads its pointer out of scope;
  ask it of every lane, a no-op where there is nothing to do.
- **Arrays and structs as values crossing a function**, returned or passed,
  and **array- or struct-typed locals assigned in a branch** and read after
  it about loops. Adreno's driver refuses such modules with
  `VK_ERROR_UNKNOWN`; a `vec2`/`vec3`/`vec4` in place of a small array, or
  the value split into its scalars, is taken. naga wrote a Rust `[u32; 2]`
  or a small struct carried through an `if` as exactly such a local. The
  compute passes by hand keep to this: forget's moved mask is read a word at
  a time inside its loop, the missed pixel's ray two functions in place of a
  nested struct. No struct holds a struct in a render module either. `Tiles`
  still crosses `tiles_age` and `tiles_claimed` by value (render build.rs),
  which no check refuses.
- **A choice made as the shader runs between two implementations that
  answer in tuples or structs.** The generated `PassObserver` chose the eye
  or the light's map in every method by the pass's head, so every answer came
  through a branch. A pass both observers take is a shader each, built from
  the same parts and the observer's own (frame/shader/part.rs `PARTS`: the
  queue's corners with beam.eye.wgsl or beam.light.wgsl, its samples with
  sample.eye.wgsl or sample.light.wgsl): choose when the shader is put
  together, not inside.
- **A table of vectors read at a place found as it runs.** The raster's
  `const array<vec3f, 72>` of box corners, indexed by its axis and vertex,
  was refused (Tint holds such a table as a local; a `var<private>` copy was
  refused too). Packed into `array<u32, 8>`, three bits a corner, and read
  back by a function (`raster_corner`, render build.rs), it is taken.
- **One shader too large.** A query as large as the world's walk asked from
  two places in one entry is written twice by a driver that inlines; split
  the entry into passes at a clean seam, as the generated `decor` split at
  `generation::placement`'s halves (footing, then pose). By hand neither half
  walks, and `decor_cs` holds both (decor_pose.wgsl).

## What the build does for the phones

Nothing a shader computes is rewritten. render build.rs strips the comments
and the indentation, appends the words it writes from pxl-engine, assembles
each module from its parts (frame/shader/part.rs `PARTS`) and validates it
with naga as WebGPU takes it, every capability off. The rules above are kept
in the hand WGSL itself, and render build.rs `held_to_faults` refuses in every
assembled module what of them naga's module shows: a struct that holds a
struct, a struct- or array-typed local, `i32::MIN` and
`atomicCompareExchangeWeak`. On 2026-11-05 the 86 modules as assembled were
about 1.0 MB, the largest (`light_beam`) 57 KB, against 5.5 MB generated;
part.rs `SHADERS` counts 92 now. The S23 has drawn with the shaders by hand
since 2026-11-05
([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md)).

## What the host and a shader share

The same S23 failed twice before, in September, with no error at all: it
compiled the shader and read a buffer laid out otherwise than the host wrote
it, so the screen drew wrong (archived minutes
`20260923-the-ground-one-device-does-not-draw.md` and
`20260927-a-matrix-the-driver-reads-its-own-way.md`, on `origin/original`).

- A `vec3f` followed by an `f32` in a struct in a `var<storage>` array: WGSL
  puts the float at byte 12, the S23's Vulkan read it at 16, every field after
  it one slot along. The terrain did not draw.
- A `var<storage> array<mat4x4f>`: read with another stride, in the native app
  (wgpu on Vulkan) as well. Every skinned character lay on the ground.

So what a `var<uniform>` or `var<storage>` holds has no room inside it: a
four-byte scalar, a `vec4` of them, an array of those whose stride is its
element, or a struct of those with nothing between its members. Never a
`vec3`, a `vec2` beside something else, a matrix, or a padded struct. Bind it
as the words it is on the wire (`array<vec4f, N>`) and read each field by a
function of its own. Never build the whole struct back from the words: Safari
on the Mac and the iPhone lost its GPU process on a shader that did, which a
field read alone did not.

Two tests held this then, parsing every module with naga
(`what_a_shader_shares_with_the_host_has_no_room_inside_it`,
`every_block_reads_each_field_from_where_it_lies`); they are not on main.
Two hand blocks break the rule today, each a `var<uniform>` holding
`mat4x4f`s (`Resolve` in screen_resolve.wgsl, `Propagate` in
screen_propagate.wgsl), unchecked on the S23.

## Check

- `ts/tool/shader/probe.html` (`pnpm probe` there) compiles every module as
  the renderer assembles it, compute and render, on the device it is opened
  on; `?bisect=<name>` finds the line it refuses, `?src=` a shader under
  `doc/experiment/`.
- Safari on the Mac stands in for the iPhone.
- The S23 over USB: `adb forward tcp:9333 localabstract:chrome_devtools_remote`
  and `adb reverse tcp:5173 tcp:5173`, then the experiment's `tools/cdp.mjs`
  compiles a WGSL file there from the Mac.
