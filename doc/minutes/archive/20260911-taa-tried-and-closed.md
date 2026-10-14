# TAA, tried and closed

**2026-09-11** — the same evening as the decor discussion. The composer had
been running 8x MSAA by accident; turning it off halved GPU p95 and brought
back shimmer on distant terrain. A temporal anti-aliasing pass was built to
answer the shimmer and then removed. The answer kept is 4x MSAA over dpr
[1, 2] supersampling.

## Why it came up

- The composer's `multisampling` defaults to 8 in @react-three/postprocessing
  3 while scene.tsx's comment said 0. With it: GPU p95 17.4 ms. Without it,
  same scene, same dpr 2: 8.1 ms.
- Without it the far terrain shimmered. Those boxes are smaller than a pixel,
  and whether a pixel lands on one changes as the view moves. MSAA had been
  resolving exactly that.
- TAA is the standard anti-aliasing in current engines, and a
  camera-reprojection variant looked cheap here: the world is nearly static,
  and render/scene_depth.ts already kept the depth and previous
  view-projection that reprojection needs.

## What was built

A `postprocessing` Pass after AO: Halton(2, 3) jitter through
`setViewOffset`, history ping-pong in half-float targets, and reprojection
by depth with the world frame's change folded in — the camera does not move
through the world here, the world moves under the camera, so
previous = prev_vp · prev_frame · cur_frame⁻¹ · cur_vp⁻¹ · current (tested).
It went through these stages:

1. **The canvas went black.** @react-three/postprocessing never calls the
   composer's `setMainCamera`, so a pass that waits for `mainCamera` never
   gets one. The pass returned without writing, the composer swapped anyway,
   and the next pass read an empty buffer. No console error: a silent return.
2. **Soft image.** It rendered at dpr 1, and it reprojected with the
   jittered matrix, so a still view resampled its history bilinearly at a
   sub-pixel offset every frame.
3. **Still views shimmered.** Stillness was judged by bit-equal matrices,
   which damped motion never reaches, and a fixed 0.9 feedback with a clamp
   kept sub-pixel detail flickering. A running mean and a disabled clip for
   still views were added — neither is part of the technique.
4. **Rewritten to the standard recipe**, the ad hoc parts removed:
   closest-depth 3×3 reprojection (Pedersen 2016, INSIDE; Tardif),
   jitter-aware source filter, Catmull–Rom history (Karis 2014), YCoCg clip
   toward the box center, luminance-weighted 0.05 feedback (Karis; Tardif),
   at dpr [1, 2] with unjittered reprojection. Distant terrain still
   jittered with a variance box (Salvi 2016, γ = 1); a rounded min/max box
   (Karis; INSIDE) did not settle it either.

## Why it was closed

TAA became the default in engines with deferred shading, where MSAA fits
badly, and it comes with a standing cost in care: motion vectors for
anything that moves, history rejection tuned against ghosting, sharpening
against softness. Each of those turned up here in turn, and each fix was
another parameter.

This renderer is forward (three.js). For a forward renderer MSAA is the
conventional answer, and the shimmer is a coverage problem, which is what
MSAA solves. The original look — dpr 2 with MSAA — was the cleanest
throughout; the only thing wrong with it was an 8x count nobody chose.

**Decided:** dpr [1, 2] as the baseline, the composer at 4x MSAA.

## Kept from the attempt

- `MainCameraLink` in scene.tsx. SceneDepthPass had the same missing camera,
  so `SCENE_DEPTH.ready` never became 1 and billboard occlusion
  (anchor_occlusion.ts) never ran. It runs now.
- A resolution switched by motion (dpr 1 while moving, 2 when still) was
  tried before TAA and set aside: it lowered quality in exactly the frames
  where shimmer shows, and needed a second rule for still frames. Its black
  frame came from R3F re-applying the Canvas `dpr` prop outside a frame
  (`if (dpr && viewport.dpr !== calculateDpr(dpr)) setDpr(dpr)` in its
  configure) and from the composer resizing only on CSS-size changes.

## What would reopen it

Needing to render below native resolution — dynamic resolution driven by GPU
time, where TAA is also the upscaler — or a move to deferred shading. Either
comes with a velocity buffer, and with the list above as the checklist.

## Sources

- Karis, "High Quality Temporal Supersampling", SIGGRAPH 2014 Advances.
- Pedersen, "Temporal Reprojection Anti-Aliasing in INSIDE", GDC 2016;
  github.com/playdeadgames/temporal.
- Salvi, "An Excursion in Temporal Supersampling", GDC 2016.
- Tardif, "Temporal Antialiasing Starter Pack", alextardif.com/TAA.html.
- Yang et al., "A Survey of Temporal Antialiasing Techniques", CGF 2020.

## Status 2026-09-13: closed, with two things held open on purpose

TAA is closed; two questions it touched are not, and are recorded here so
"closed" is not read as "settled".

- **The cross-fade is wanted.** The complementary stochastic handoff
  between adjacent LOD tiers was tangled with the TAA attempt, because
  the temporal filter was one way to hide its noise. Dropping TAA does not
  drop the cross-fade: it is the right handoff and stays, and whatever
  anti-aliasing is in place has to be judged with it on.
- **MSAA is a cost that will be revisited.** It came in for one reason —
  distant terrain shimmering — and the TAA attempt was the reach for a
  cheaper answer to that same shimmer, which it did not give. So today's
  MSAA is a solution to the jitter that pays in memory (the phone
  measurements in
  [20260912-why-the-destination-is-wasm.md](../20260912-why-the-destination-is-wasm.md)),
  not a decision that the memory is worth it forever. A method that takes
  both the shimmer and the memory — and there are cheaper shapes than a
  full temporal filter — is to be tried on the desktop web first, where
  it can be seen, and only then judged as a replacement. Not on the
  mobile path until it has earned it on the desktop one.

  Measured on the WebGPU renderer, 2026-09-14, M1 Max, a 9.6 Mpx canvas
  (dpr 2), 4x MSAA on, shadows off: the color and depth targets 147 MB
  each (the ledger's `targets` line), GPU 10.6 ms p50 / 20.1 ms p95 by
  timestamp query, dt p50 16.5 ms against 8.3 ms before the post effects,
  36% of frames missed — the frame is GPU-bound at that pixel count, which
  is where MSAA pays in bandwidth, not memory. Not decided on that one
  capture; the comparison that separates the costs, still to be taken on
  the same machine and view:
  1. MSAA off, shadows off — the baseline: reversed-Z, mipmaps, the rim
     light and the vignette only.
  2. MSAA off, shadows on — what the shadow map costs: its depth pass and
     the 3x3 PCF.
  3. MSAA on, shadows on — the full set.

  Both are settings (quality → MSAA recreates the renderer; lighting →
  shadows), so the three are one ledger capture each. What the numbers
  turn on: whether the desktop default stays MSAA on, becomes pixel-count
  aware (on under a few megapixels, off above), or waits for the cheaper
  method above.
