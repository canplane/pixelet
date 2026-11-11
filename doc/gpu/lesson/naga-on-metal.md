# naga on Metal

**2026-11-05.** What wgpu's own shader translator, naga, does wrong on Metal,
and what the WGSL keeps clear of for it. Wherever wgpu runs on an Apple GPU,
naga translates the WGSL to Metal: the native apps on iOS, visionOS and the
Mac, and the pxl-render tests run on this Mac (`cargo test -p pxl-render`).
The page in a browser does not go through naga: Chrome compiles WGSL with
Tint and Safari with WebKit's compiler
([shaders the phones take](./shaders-phones-take.md)).

## Keep clear of

- **`break if` in a loop's `continuing`.** In
  `loop { … continuing { break if cond; } }`, naga's Metal backend writes a
  `let` of the loop's body that the `break if` reads as its expression again,
  evaluated after the counter has moved, so the loop ends one iteration early
  (gfx-rs/wgpu#4558). The fix, gfx-rs/wgpu#10459, is on wgpu's trunk; neither
  the release 30.0.1 nor the fork the tree takes wgpu from carries it
  (`rust/wgpu-fork.md`).
- The hand WGSL (`rust/client/render/src/frame/shader/`) has no `continuing`
  block and no `break if`: a loop ends by a `for` condition or a `break` in its
  body. Check with `grep -rn 'continuing\|break if' rust/client/render/src/frame/shader`.
- A loop of that shape would show in the reference tests on this Mac (they
  run through naga on Metal) as a walk or a count one step short of
  pxl-engine's; the page would not show it.

## How it was met

On 2026-10-14, when the GPU first cast a ray a pixel: the generated shaders'
`node_seed` loop ended early on the native path, and was unrolled for it
([the GPU casts a ray a pixel](../../minutes/20261014-the-gpu-casts-a-ray-a-pixel.md),
"A known break on the device path"). The tree then pinned wgpu's crates to a
rev holding the fix, and took the pin out on 2026-11-05, as no WGSL here has the
shape. Since 2026-11-10 the tree takes every wgpu crate from the fork
`canplane/fork.wgpu` (`rust/Cargo.toml` `[patch.crates-io]`, `rust/wgpu-fork.md`),
the user having allowed a patch where the fault is wgpu's
([a device is not a page](../../lesson/a-device-is-not-a-page.md)).

## If a loop of that shape is needed before a release carries the fix

Put gfx-rs/wgpu#10459 on the fork's branch as a commit of its own, list it in
`rust/wgpu-fork.md`, and move the rev in `rust/Cargo.toml`'s `[patch.crates-io]`.
Then `make -j8 test-gpu`.
