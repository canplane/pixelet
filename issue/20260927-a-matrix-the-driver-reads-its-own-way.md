# A matrix the driver reads its own way

On a Galaxy S23, in the device app, the characters and their pets lay on the
ground — stretched diagonally away from the eye, head far off, feet near —
while the decor beside them stood, and while an iPhone and Chrome on a Mac
drew the same actors upright. Nothing reported an error.

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
and the shader share, whose layout the backend has to spell out and a driver
has to read back.

## What the device was

|        |                                                                     |
| ------ | ------------------------------------------------------------------- |
| failed | Galaxy S23 (Adreno 740), One UI 8.5, the device app, wgpu on Vulkan |
| drew   | iPhone 13 mini (wgpu on Metal), Chrome on a Mac (Dawn)              |

## How it was narrowed

Each line is a hypothesis and the cut on the device that ended it. Every cut
was a change to the real shader or the real kernel, installed as a release
build, looked at, and reverted — the lesson of 20260923, that a standalone page
cannot find a fault that exists only in the real declaration.

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

The last two lines together named the fault: the words in the buffer were
right and the matrices the shader assembled from them were not. Read as
`array<vec4f>` and assembled column by column in the shader, the same buffer
drew every actor upright, arms down, in its idle clip.

## Why it passes everywhere else

WGSL defines one memory layout. The shader does not run as WGSL: naga or Tint
translates it into SPIR-V, MSL or HLSL, spelling the layout out as decorations
— `Offset`, `ArrayStride`, `MatrixStride`, `ColMajor` — and each driver reads
those back. Vulkan alone has four layout rules a block may follow (std140,
std430, relaxed, scalar), and the types where they differ are the ones with
room inside them: a `vec3` that is twelve bytes aligned to sixteen, a matrix
whose columns have a stride, a struct in an array that is padded to its
alignment. A driver that reads one of those differently reads everything after
it one slot along — or, for a matrix, reads its columns at the wrong stride.

This is not only one vendor's. wgpu has an open report of a `mat4` storage
array reading as nearly all zeros on an NVIDIA card under Windows
(gfx-rs/wgpu#914); RenderDoc has one of std430 `mat4x3` array padding decoded
wrong (baldurk/renderdoc#2378). The Vulkan Guide's chapter on shader memory
layout lists where the rules part.

A `vec4`, a four-byte scalar, and an array of either have no room inside them
and one stride. There is nothing about them for two readers to disagree on.

## What changed

- **The palette is `array<vec4f>`**, four a joint, and `joint(j)` makes the
  matrix from them in the shader. The kernel writes the same sixty-four bytes
  a joint as before; only the declaration changed.

## What is proposed

The fix above closes one declaration. The class is every declaration of its
kind, so the proposal is a rule and a check.

**The rule.** What the host and a shader share — every `var<uniform>` and
`var<storage>` — is made of types with no room inside them:

- a four-byte scalar (`f32`, `u32`, `i32`), a sixteen-byte vector (`vec4f`,
  `vec4u`, `vec4i`), an array of either, or a struct of them;
- never a `vec3` or a `vec2` in a struct, and never a matrix;
- a matrix is assembled in the shader from `vec4` columns, and a three-vector
  is a `vec4`'s `.xyz`.

Types a shader keeps to itself — `Seen`, `Air`, what one function hands
another — are no one else's to read and stay as they are.

**The check.** A rule kept by memory is kept until the next shader. naga,
which wgpu already carries, parses every shader module; a test walks each
uniform and storage global's type to its leaves and refuses anything outside
the list, beside `every_pipeline_matches_its_shader`. A `mat4x4f` added to a
uniform then fails `deno task test`, on every machine, before any device sees
it.

**What it touches today.** Storage passes after this fix: what remains is
`u32`, `f32` and `vec4f` arrays and two structs of `vec4`s (`Shape`, the
chunk's `Draw`). Uniforms do not: `Frame` (a `mat4x4f` and several `vec3f`),
`Atmosphere` (eight `vec3f`), `Clouds` (a `mat3x3f`, whose columns carry
padding — the riskiest of them), `Ao` and `Badge` (`mat4x4f`s), and the `Draw`
of decor, debug, the puck and the actors, each opening on a `vec3f`. They draw
right on the S23 today, which is luck, not a property; each becomes `vec4`
fields, with the Rust that writes them.

## Where it stands

The palette is fixed and verified on the S23. The rule and its check are
proposed and not yet applied.
