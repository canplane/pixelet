# A matrix the driver reads its own way

On a Galaxy S23, in the device app, the characters and their pets lay on the
ground — stretched diagonally away from the eye, head far off, feet near — while
the decor beside them stood, and while an iPhone and Chrome on a Mac drew the
same actors upright. Nothing reported an error.

It was the joint palette. The skinned shader declared it

```wgsl
@group(3) @binding(0) var<storage, read> palette: array<mat4x4f>;
```

and the S23's Vulkan read that array as some other layout than the one the
kernel wrote. The decor has no matrix in storage — its instances are an
`array<u32>` — which is why it stood.

This is the second time on the same device. The first
(20260923-the-ground-one-device-does-not-draw.md) was a `vec3f` followed by a
float in a struct inside a storage array: WGSL puts the float at twelve, the
device put it at sixteen. Both are the same class: a type in a buffer the host
and the shader share, whose layout the backend has to spell out and a driver has
to read back.

## What the device was

|        |                                                                     |
| ------ | ------------------------------------------------------------------- |
| failed | Galaxy S23 (Adreno 740), One UI 8.5, the device app, wgpu on Vulkan |
| drew   | iPhone 13 mini (wgpu on Metal), Chrome on a Mac (Dawn)              |

## How it was narrowed

Each line is a hypothesis and the cut on the device that ended it. Every cut was
a change to the real shader or the real kernel, installed as a release build,
looked at, and reverted — the lesson of 20260923, that a standalone page cannot
find a fault that exists only in the real declaration.

|                                                  |                                                                                                                                                             |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| the palette never reaching the GPU               | The palette filled with zeros where it had been identity before the first tick: the actors still drew, lying as before. The rewrites land.                  |
| the joints attribute read wrong                  | Colored by joint index: every part its own index, all in range, `y`, `z`, `w` zero.                                                                         |
| the weights attribute read wrong                 | Colored by weight: (1, 0, 0) everywhere, as the rigid models are.                                                                                           |
| the matrix blend (`mat * scalar + mat * scalar`) | Blended as four matrix-vector products instead: unchanged.                                                                                                  |
| the palette the kernel computes                  | Dumped on the device: near identity at rest, the arms turned for the idle clip — right.                                                                     |
| the palette's memory being overwritten           | Read back at `draw()`, after every tick, against a fresh `global * inverse_bind`: identical, and no actor's range overlapping another's.                    |
| the draw uniform's `scale`                       | Colored by it: 1.4 for the character, 0.2 for the pet, sane.                                                                                                |
| **the palette as the shader reads it**           | **Drawn without it, the character stood in its bind pose.** Colored by its translation column, the GPU read values of 0.18–0.34 where the memory held zero. |

The last two lines together named the fault: the words in the buffer were right
and the matrices the shader assembled from them were not. Read as `array<vec4f>`
and assembled column by column in the shader, the same buffer drew every actor
upright, arms down, in its idle clip.

## Why it passes everywhere else

WGSL defines one memory layout. The shader does not run as WGSL: naga or Tint
translates it into SPIR-V, MSL or HLSL, spelling the layout out as decorations —
`Offset`, `ArrayStride`, `MatrixStride`, `ColMajor` — and each driver reads
those back. Vulkan alone has four layout rules a block may follow (std140,
std430, relaxed, scalar), and the types where they differ are the ones with room
inside them: a `vec3` that is twelve bytes aligned to sixteen, a matrix whose
columns have a stride, a struct in an array that is padded to its alignment. A
driver that reads one of those differently reads everything after it one slot
along — or, for a matrix, reads its columns at the wrong stride.

This is not only one vendor's. wgpu has an open report of a `mat4` storage array
reading as nearly all zeros on an NVIDIA card under Windows (gfx-rs/wgpu#914);
RenderDoc has one of std430 `mat4x3` array padding decoded wrong
(baldurk/renderdoc#2378). The Vulkan Guide's chapter on shader memory layout
lists where the rules part.

A `vec4`, a four-byte scalar, and an array of either have no room inside them
and one stride. There is nothing about them for two readers to disagree on.

## What changed

- **The palette is `array<vec4f>`**, four a joint, and `joint(j)` makes the
  matrix from them in the shader. The kernel writes the same sixty-four bytes a
  joint as before; only the declaration changed.

## The rule for the class

The fix above closes one declaration. The class is every declaration of its
kind, so it took a rule and a check.

**The rule.** What the host and a shader share — every `var<uniform>` and
`var<storage>` — is made of types with no room inside them: a four-byte scalar,
a four-lane vector of them, an array of those whose stride is its element, or a
struct of those whose members follow one another with nothing between. Never a
`vec3`, a `vec2` beside something, a matrix, or a padded struct. Types a shader
keeps to itself — `Seen`, `Air`, what one function hands another — are no one
else's to read and stay as they are.

**How a block keeps it.** Thirteen uniforms broke it: `Frame`, `Atmosphere`,
`Clouds` (a `mat3x3f`, whose columns carry padding), `Sky` (another), `Ao`,
`Badge`, `Stars`, the `Draw` of decor, debug, the puck and the actors, and the
sheet's `Entry`. Each is now bound as what it is on the wire, `vec4` words:

```wgsl
struct Frame { view_proj: mat4x4f, camera_pos: vec3f, edge_strength: f32, … };
@group(0) @binding(0) var<uniform> frame_words: array<vec4f, 21>;
fn frame_view_proj() -> mat4x4f { return mat4x4f(frame_words[0], frame_words[1], frame_words[2], frame_words[3]); }
fn frame_camera_pos() -> vec3f { return frame_words[4].xyz; }
fn frame_edge_strength() -> f32 { return frame_words[4].w; }
…
```

and read as `frame_camera_pos()` where it was `frame.camera_pos`. The struct
stays as it was, the layout the functions are held to, so the tests that hold
the Rust's offsets against it (`frame_layout_matches_the_struct`,
`the_skys_layouts_match_their_structs`) keep their meaning, and the bytes the
Rust writes did not move: every field already sat where the words put it.
`Entry`, one `vec2f` in a sixteen-byte buffer, became a `vec4f` whose `xy` is
the size.

The other way considered was to rename the fields to the words
(`frame.camera.xyz`, `frame.camera.w` for the edge strength) and drop the
struct. It loses every name a shader reads, some two hundred of them — and the
functions are where the layout is written down once, beside the words.

A field is read by a function of its own, never by one that builds the whole
struct back from the words. The first version did (`fn frame() -> Frame`, read
as `frame().camera_pos`), and Safari's WebGPU, on a Mac and on an iPhone, lost
its GPU process on the terrain's pipeline: every tab's device went with it, and
no adapter answered for a while after. Cut on the real shader, one pipeline and
one block at a time: the terrain's alone killed it, with either of its blocks
built back; built once per invocation it still died; read field by field,
nothing did. Chrome, the S23 and the iPhone app compiled it either way. A
field's function is also a single load where the builder was a struct's worth of
them for one field.

**The check.** A rule kept by memory is kept until the next shader. Two tests
beside `every_pipeline_matches_its_shader` parse every module with naga:

- `what_a_shader_shares_with_the_host_has_no_room_inside_it` walks every uniform
  and storage global's type to its leaves and refuses a gap anywhere;
- `every_block_reads_each_field_from_where_it_lies` follows each field's
  function back to the words, holds it to the word and lane WGSL lays that field
  at — a lane off by one in `frame_screen_ao()` fails as `Frame.screen_ao` — and
  refuses a function that builds the struct back.

Both were checked against the fault they are for: the palette put back as
`array<mat4x4f>` fails the first, and one lane moved in a field's function fails
the second.

## Where it stands

Applied. On the S23 the ground, the sky, the decor, the actors and the puck draw
as they should; in Chrome on a Mac the ground, the sky and the decor do. Not yet
looked at: the actors in Chrome (the tab had no location, so no character stood)
and the iPhone.
