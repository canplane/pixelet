# WGSL

What the WGSL language itself does and lacks, whichever compiler takes it.
What each compiler or driver refuses is in [shaders the phones take](./shaders-phones-take.md)
and the implementations' files.

- **What the language lacks.** No u64, i64, f64, u8 or i16, no infinity, no
  recursion and no pointer arithmetic. As of 2026-11-03 no mature path from
  another language to WGSL existed.
  ([the GPU casts a ray a pixel](../../minutes/20261014-the-gpu-casts-a-ray-a-pixel.md):57-58;
  [Rust and XTP reconsidered](../../minutes/20261103-rust-and-xtp-reconsidered.md):207-245;
  [what the published build gives away](../../minutes/archive/20260918-what-the-published-build-gives-away.md):216-220)
- **`select` evaluates both arguments.** A side effect inside either always
  runs (155 wrong of 18,677).
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):491-494, 615-617)
- **Transcendentals.** `sin` and `atan` are bounded only to an absolute 2^-11,
  so a small angle times the planet's radius is metres off. The class of error is
  in [cpu-and-gpu-agree](./cpu-and-gpu-agree.md).
  ([the sky is the air](../../minutes/archive/20260922-the-sky-is-the-air.md):130-135)
- **f32 at the planet's scale.** An f32's ulp is about 2 m at Web Mercator's
  extent, so GPU coordinates are taken relative to the eye.
  ([coordinate system](../../minutes/archive/20260811-coordinate-system.md):16-19;
  [every size is a zoom](../../minutes/archive/20261006-every-size-is-a-zoom.md):46-53)
- **`f32::MAX` printed by naga** is a literal Tint reads as past f32's range,
  and a float past that range failed a frame silently.
  ([the cone touches](../../experiment/20261028-the-cone-touches.md):92;
  [a ray from the box](../../experiment/20261026-a-ray-from-the-box.md):64-65)
- **naga's refusals.** naga refuses a module that defines a function twice, and
  a pointer-typed local (`LocalVariable InvalidType`). An atomic in a branch of
  its own is in [shaders the phones take](./shaders-phones-take.md).
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):1045-1048;
  [the cone touches](../../experiment/20261028-the-cone-touches.md):89-91)
- **Debugging a layout fault.** A layout fault belongs to a declaration, so a
  standalone page passes; cut the real shader down on the device.
  ([the ground one device does not draw](../minutes/archive/20260923-the-ground-one-device-does-not-draw.md):127, 138-158;
  [a matrix the driver reads its own way](../minutes/archive/20260927-a-matrix-the-driver-reads-its-own-way.md):34-37)
- **Stripping the text.** Stripping comments costs nothing; stripping
  whitespace saved 392 bytes gzipped and loses the line numbers a driver reports
  against.
  ([what the published build gives away](../../minutes/archive/20260918-what-the-published-build-gives-away.md):222-226, 253-258)
