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
  (gfx-rs/wgpu#4558). The fix, gfx-rs/wgpu#10459, is on wgpu's trunk; the
  latest release, 30.0.1, does not carry it (as of 2026-11-05).
- The hand WGSL (`rust/client/render/src/frame/shader/`) has no `continuing`
  block and no `break if`: a loop ends by a `for` condition or a `break` in its
  body. Check with `grep -rn 'continuing\|break if' rust/client/render/src/frame/shader`.
- A loop of that shape would show in the reference tests on this Mac (they
  run through naga on Metal) as a walk or a count one step short of
  pxl-engine's; the page would not show it.

## How it was met

On 2026-10-14, when the GPU first cast a ray a pixel: the generated shaders'
`node_seed` loop ended early on the native path, and was unrolled for it
([the GPU casts a ray a pixel](../minutes/20261014-the-gpu-casts-a-ray-a-pixel.md),
"A known break on the device path"). The tree then pinned wgpu's crates to a
rev holding the fix. On 2026-11-05 the pin came out: wgpu is followed as it is
released, never forked (the user), and no WGSL here has the shape. On
2026-11-09 the user allowed a patch where the fault is wgpu's
([a device is not a page](./a-device-is-not-a-page.md)).

## If a loop of that shape is needed before a release carries the fix

Pin wgpu's crates to a rev that holds it, in `rust/Cargo.toml`, as the tree did
until 2026-11-05. Trunk's naga-types no longer matched 30.0.1, so every wgpu
crate moved to the one rev:

```toml
[patch.crates-io]
naga = { git = "https://github.com/teoxoy/wgpu", rev = "fb9c4542d81d1db96ad5cf3295214c7ba91034ca" }
naga-types = { git = "https://github.com/teoxoy/wgpu", rev = "fb9c4542d81d1db96ad5cf3295214c7ba91034ca" }
wgpu = { git = "https://github.com/teoxoy/wgpu", rev = "fb9c4542d81d1db96ad5cf3295214c7ba91034ca" }
wgpu-core = { git = "https://github.com/teoxoy/wgpu", rev = "fb9c4542d81d1db96ad5cf3295214c7ba91034ca" }
wgpu-core-deps-apple = { git = "https://github.com/teoxoy/wgpu", rev = "fb9c4542d81d1db96ad5cf3295214c7ba91034ca" }
wgpu-hal = { git = "https://github.com/teoxoy/wgpu", rev = "fb9c4542d81d1db96ad5cf3295214c7ba91034ca" }
wgpu-naga-bridge = { git = "https://github.com/teoxoy/wgpu", rev = "fb9c4542d81d1db96ad5cf3295214c7ba91034ca" }
wgpu-types = { git = "https://github.com/teoxoy/wgpu", rev = "fb9c4542d81d1db96ad5cf3295214c7ba91034ca" }
```

Then `cargo update --offline -p` for those crates (Cargo.lock), and the
pxl-render tests. Drop it again when a wgpu release carries #10459. With the
pin, the lock held wgpu 30.0.0 from git and bit-set 0.11.1, bit-vec 0.10.1;
without it, 30.0.1 from crates.io and bit-set 0.10.0, bit-vec 0.9.1.
