# Shaders the phones take

**2026-11-03.** What a compute shader keeps clear of so the phones' browsers
compile it, learned on a Galaxy S23 (Adreno 740, Chrome) and an iPhone 13
mini (Safari 27), with Safari on the Mac failing as the iPhone does. The
evidence, the reduced cases and the tools are
[the shaders on two phones](../experiment/20261103-shaders-on-two-phones/).
These hold whatever language the shaders are written in: they are about the
WGSL a browser receives and what its compiler and the phone's driver do with
it, not about Rust.

Desktop Chrome passing proves little here. Chrome on the Mac compiles WGSL
with Tint to Metal for a desktop GPU; Safari compiles it with WebKit's own
compiler; Chrome on Android compiles it with Tint to SPIR-V for the phone's
own Vulkan driver. Each is a different compiler with its own faults.

## Keep clear of

- **`atomicCompareExchangeWeak`.** Safari writes Metal for it that does not
  compile, on the iPhone and the Mac alike. Claim with an exchange and put
  back what was not yours (pxl-shader pyramid.rs `pyramid_claim`); take from
  a counter with an add and give back with a min where it did not fit
  (way.rs `pop`). Never give back with a subtract while others take: it
  overlaps what they took.
- **`i32::MIN` written as a constant.** Safari writes it as a bare literal,
  a C++ `long`, and a `select` of it and an `int` fails. pxl-shader-build
  writes it as a call (`i32_min_called`).
- **An atomic in a branch of its own.** naga reads its pointer out of scope;
  ask it of every lane, a no-op where there is nothing to do.
- **Arrays and structs as values crossing a function**, returned or passed,
  and **array- or struct-typed locals assigned in a branch** and read after
  it about loops. Adreno's driver refuses such modules with
  `VK_ERROR_UNKNOWN`; a `vec2`/`vec3`/`vec4` in place of a small array, or
  the value split into its scalars, is taken. naga writes a Rust `[u32; 2]`
  or a small struct carried through an `if` as exactly such a local.
- **One shader too large.** A query as large as the world's walk asked from
  two places in one entry is written twice by a driver that inlines; split
  the entry into passes at a clean seam, as `decor` splits at
  `generation::placement`'s halves (footing, then pose).

## Check

- `ts/app/web/probe.html` on the dev server compiles every module on the
  device it is opened on; `?src=` a shader under `doc/experiment/`.
- Safari on the Mac stands in for the iPhone.
- The S23 over USB: `adb forward tcp:9333 localabstract:chrome_devtools_remote`
  and `adb reverse tcp:5173 tcp:5173`, then the experiment's `tools/cdp.mjs`
  compiles a WGSL file there from the Mac.
