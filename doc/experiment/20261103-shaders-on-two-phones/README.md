# The shaders on two phones

**2026-11-03.** Why the page draws nothing on a Galaxy S23 and an iPhone 13
mini, shader by shader, on main at `feee29054` (the generated WGSL of
`4a73ebdd0`, `i32::MIN` already a call). Two faults, one each: the S23's
driver refuses to make pipelines; Safari's WGSL compiler writes Metal that
does not compile. Neither is ours, and each is worked around here only as far
as an experiment; what is adopted is the user's.

## How

- **S23** (SM-S911N, Adreno 740 as `adapter.info` says `qualcomm ·
  adreno-7xx`), Chrome 156 on Android, Vulkan. **iPhone 13 mini**, Safari 27.0.1.
  **Mac**, Safari, which fails the same way the iPhone does.
- The dev server on :5173. The phones reach it through a Cloudflare quick
  tunnel, or the S23 over USB (`adb reverse tcp:5173`), where `localhost` is a
  secure context.
- `ts/app/web/probe.html` (`src/probe.ts`, dev only) makes each module under
  `rust/client/render/src/frame/shader/` a pipeline on the device alone and
  says what the device said: `?only=` some, `?bisect=` a module cut by
  `return;`, `?src=` a shader under `doc/experiment/`. Each line also reaches
  the dev server's console (`src/remote-console.ts`), into
  `ts/app/web/.dev-console.log`.
- The S23's Chrome driven over the DevTools protocol (`adb forward tcp:9333
  localabstract:chrome_devtools_remote`): `tools/cdp.mjs` sends a WGSL file
  from the Mac, makes it a compute pipeline there, and prints what came back.
  `tools/reduce.mjs` shrinks a file while the device still refuses it (a
  block emptied, a loop removed, an assignment, a dead `let`, a condition made
  constant); `tools/guard.py` first bounds every loop to 1000 turns, so a cut
  cannot leave a loop that never ends. `tools/descend.py` empties one block at
  a time; `tools/fnbisect.py` one function at a time.
- The rewrites tried on the generated WGSL: `tools/split.py` (each
  array- or struct-typed `phi_*` local a variable per leaf), `tools/forward.py`
  (a constant access into a composite `let` replaced by its component, per
  function), `tools/vecify.py` (`array<u32|i32|f32, 2..4>` as a vector,
  outside the structs a binding reaches). The always-inlined set is
  `pxl-shader-build` with `wgsl_of` made to pick `inlined`.

## The S23

Every module is valid WGSL (Tint takes each, as `src/shader.test.ts` checks);
the driver fails nine compute pipelines and one render pipeline in
`vkCreateComputePipelines` / `vkCreateGraphicsPipelines` with
`VK_ERROR_UNKNOWN`, made async or not:

| fails | takes |
|---|---|
| beam, cell, decor, face, light_miss, light_propagate, miss, miss_list, sample_box; one `vs+fs` not yet named | the 21 other compute modules |

No WGSL feature sets the two apart: size (cell is 30 KB and fails, sample is
380 KB and is taken), loops, atomics, the `array<vec4<i32>, 46>` uniform,
`vec2<u32>` members. Reduced on the device:

- `cell` empties to a pair of nested loops whose exits ride on locals, after
  an `array<u32, 2>` local set in one branch of an `if`, its elements compared
  (`repro/adreno-refused.wgsl`, 174 lines, every loop bounded). The same with
  that local a `vec2<u32>` is taken (`repro/adreno-taken.wgsl`). So is it with
  the array computed in place instead of carried in the local.
- With the arrays gone, the next refusal is a struct-typed local carried the
  same way (`type_30`); split into its two leaves, taken.
- With the locals split, the next is a composite *value*: the leaves
  reassembled (`let _e424 = type_30(array<u32, 2>(…), …)`) and read by member.
- In the modules that keep their functions, the next is a function returning
  a composite (`-> array<u32, 2>`, a struct holding arrays); emptying either
  of two such functions makes `beam` taken.
- `decor` is refused whatever is rewritten, and taken once any of ten
  unrelated functions is emptied, a buffer read of 8 lines among them: no
  shape, a size or a load. Split in two at `generation::placement`'s halves
  (the column surfaces, then the pose about them; in a worktree), each half is
  taken, inlined, rewritten or not; with its functions kept, each is refused.

The rewrites over every module, on the S23:

| set | refused |
|---|---|
| as generated | the nine above |
| always inlined | beam, cell, light_propagate, sample_box |
| inlined, split, forwarded, vectors | way |
| inlined, split, vectors | cell, way_tree |

A rewrite that takes one module refuses another the set took before, so no
one set of these is a fix. Compile times on the S23 run to 2–5 s a large
module, 12 s rewritten.

## The iPhone, and Safari on the Mac

Safari's WGSL compiler (WebKit) writes Metal for every module, and Metal
refuses the six that reach `atomicCompareExchangeWeak` (pxl-shader
pyramid.rs `pyramid_swap`, way.rs `pop`): beam, sample, miss, miss_list,
light_miss, way_tree.

```
program_source:47:7: error: field may not be qualified with an address space
    T old_value;
program_source:54:12: note: in instantiation of template class
'__atomic_compare_exchange_result<thread unsigned int>' requested here
    return __atomic_compare_exchange_result<decltype(compare)> { innerCompare, exchanged };
```

The helper WebKit writes takes the result's type from `decltype(compare)`, a
by-value parameter, which Metal's compiler gives an address space. Nothing in
the WGSL reaches it: `repro/webkit-cas.wgsl`, one call, fails the same way in
Safari on the Mac (`probe.html?src=20261103-shaders-on-two-phones/repro/webkit-cas.wgsl`)
and is taken on the S23.

Before `4a73ebdd0` Safari also refused a `select` of `i32(-2147483648)` and an
`int`: WebKit writes that constant as the bare literal, a C++ `long`.

## Reports to file

Drafts; not sent.

### Chrome (Dawn) / Qualcomm: Adreno 740 refuses a valid compute pipeline

> **Summary.** On a Galaxy S23 (Adreno 740), Chrome 156 rejects
> `createComputePipelineAsync` for a valid WGSL compute shader with
> `OperationError: CreateComputePipelines failed with VK_ERROR_UNKNOWN`. The
> same module compiles on desktop Chrome (Metal, Vulkan) and with Tint
> standalone.
>
> **Reduced case.** `adreno-refused.wgsl` (attached, 174 lines): a function-scope
> `var` of type `array<u32, 2>`, assigned in one branch of an `if` and its
> elements compared, followed by two nested `loop`s whose exits are carried
> in `bool` locals (`break if`). Changing that one local to `vec2<u32>`
> (`adreno-taken.wgsl`) makes the pipeline succeed. Struct-typed locals and
> functions returning arrays or structs fail the same way in the full shaders.
>
> **Device.** SM-S911N, Android (Chrome reports `Android 10; K`), Chrome
> 156.0.8078.73, `adapter.info`: vendor `qualcomm`, architecture `adreno-7xx`.
>
> **Expected.** The pipeline is created, or a validation error names the
> cause. A Dawn workaround for Qualcomm (e.g. scalarizing function-scope
> composites before SPIR-V) would cover pages that cannot change the shape.

### WebKit: `atomicCompareExchangeWeak` produces Metal that does not compile

> **Summary.** Any compute shader calling `atomicCompareExchangeWeak` fails
> to compile in Safari 27.0.1 on iOS and in Safari on macOS: "Failed to
> compile the shader source, generated metal", `error: field may not be
> qualified with an address space` in the helper
> `__wgslAtomicCompareExchangeWeak`, instantiated as
> `__atomic_compare_exchange_result<thread unsigned int>` from
> `decltype(compare)`.
>
> **Reduced case.** `webkit-cas.wgsl` (attached, 11 lines).
>
> **Expected.** The helper takes its result type from the template parameter
> `S` (or `remove_address_space_t<decltype(compare)>`), not from
> `decltype` of the parameter.

## Open

- The `vs+fs` render pipeline the S23 refuses, not yet named (the page
  composes it from hand-written modules at run time).
- Whether a module the S23 takes computes what the Mac computes: an earlier
  S23 read buffers laid out differently and drew wrong without an error
  (archived minutes of 2026-09-23 and 2026-09-27, on `origin/original`).
- The native app (wgpu: naga to SPIR-V or Metal) on the same phones.
