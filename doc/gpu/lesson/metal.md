# Metal

Apple's GPUs under Metal, on the Mac and the iPhone, whichever implementation
drives them. Floats on Metal (fused sums, division, edges across pipelines) are
in [cpu-and-gpu-agree](./cpu-and-gpu-agree.md); a GPU job that holds the whole
Mac is in [the GPU is shared](./the-gpu-is-shared.md).

- **Half speed.** The Mac's GPU sometimes ran at half speed across every span, a
  state no thermal or power record showed, and such runs are left out
  ([the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):812-815).
  Runs alone differ by about 4 ms (789-791), another GPU application or other load
  spoils a run (428-431; [the lights' tail](../../experiment/20261107-the-lights-tail/README.md):235-236, 313-314),
  and a display with another pixel ratio or refresh rate does not compare
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):204-205).
- **A split pass on a tiler** stores and reloads its tiles each frame. A rise
  once taken for that cost was spans overlapping.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1262-1264;
  [the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):862-867, 930-934)
- **The iPhone's memory** is `memory-physical-footprint`, recorded after the
  launch ([a device is not a page](../../lesson/a-device-is-not-a-page.md)). It
  does not compare with Android's `Graphics`.
  ([the app that did not move](../../experiment/20261109-the-app-that-did-not-move/README.md):24-27, 127-128;
  [one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):33-36)
- **Discard with depth written.** A fragment shader that discards and writes
  depth is stencil-tested only after shading on the Mac, so a stencil does not
  gate it; a depth gate rejects early. Whether discard keeps the early test on
  Adreno and Mali is unmeasured: one passage of the minutes says it is lost on
  tilers, another that it is kept, inferred from Mesa.
  ([the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):113-115, 163-178;
  [the screen keeps its tiles](../../minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md):365-368, 512-516, 532-536)
