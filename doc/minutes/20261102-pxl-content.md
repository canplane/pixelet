# pxl-content, pixelet's content beside the engine

**2026-11-02** — recorded with the user, through the coordinating session.
Main was at `ca7aa2626d`; the commits that carry this record land it. The
commits are named as `original` keeps them.

## What was asked

Split pixelet's content out of pxl-spec-core into a new gpu-portable crate,
`pxl-content`, behind one trait, so the engine never names content.
pxl-spec-core kept its name and path for the split; it was renamed to
pxl-engine separately (`283ded8dc2`), and this record uses the new name.

## What the user decided

- **A new crate `pxl-content`**, `no_std` and gpu-portable, beside
  pxl-engine, which depends on no content crate (Cargo holds it).
- **One trait, `Content`, in pxl-engine, with rules only**: the caves
  (`hollow`, `all_hollow`), the strata (`by_depth`, `bedrock`, `ceiling`,
  `earth`), and the decor's bounds the light asks (`TALLEST`, `WIDEST`). One
  zero-sized implementation, `Pixelet`, in pxl-content. It is carried as an
  associated type on the trait a world type already implements where a rule
  is asked (`ground::Resident`), no new type parameter on `world::node` or
  `ground::node`.
- **The value space is the engine's** (pxl-engine value.rs): the
  sentinels (void 0, unknown 0xff, air 256) and the slots each class of value
  takes (the ground's entries 1 to 254, the ways' 37 to 55, the pier 35, the
  object's colors 257 to 1023), compile-time constants. pxl-content's palette
  is laid into them, every value where it is, as baked tiles hold them, and
  checked at compile time. No content type parameter spreads to the fill, the
  ways' tree, overwrite or the light's other functions for classification.
- **Verification** is the WGSL's size, naga's validation and the tests, where
  it was first byte-identity: moving code between crates changes rust-gpu's
  numbering and which functions pxl-shader-build keeps. The split's own
  growth is accepted: a few KB per module, from rustc's copy of each small
  helper in every crate that calls it. Since 2026-11-05 the WGSL is written by
  hand and held to pxl-engine word for word
  ([the shaders by hand](../gpu/minutes/20261105-the-shaders-by-hand-held-to-the-engine.md)).
- **The decor's scatter, placement and generation and the palette files move
  as files**, no behaviour changed. A refusal of a changed palette id in the
  generator was dropped from scope.

## Rejected

- Classification as trait methods (`grounds`, `transports`, `objects`,
  `PIER`) and the sentinels on `Content`: every function reading them would
  have taken a content type parameter (the fill, the ways' tree, overwrite,
  cast's walk); and generic code changed the WGSL's bytes.
- Byte-identical WGSL at every step: rustc's per-crate copies and the
  call-or-inline choice make it unreachable for any code that changes crate.
- Merging identical functions in pxl-shader-build before the call-or-inline
  choice, with or without leaving labels and returns out of `call_shorter`'s
  count: against `fcf561cf92` the merge alone was −37,264 bytes in all but
  light_slide +1,554 and way_tree +222; with the count fix −50,937 but light
  +3,531, way_tree +797 and height +34. The kept set is chosen on each function's instruction count before
  spirv-opt and the module is taken whole with those boundaries or with
  none, so any change to the counts moves the set and some module grows; two
  fixes that did not converge ended the attempt.
