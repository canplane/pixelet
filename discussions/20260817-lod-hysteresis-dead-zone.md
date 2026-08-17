# LOD Hysteresis Dead Zone at the 1536 Threshold

**2026-08-17** — investigation into a reported inconsistency between how LOD
sharpens and how it coarsens, and whether it costs real render performance.
No code changed; this is a report, not a fix.

## The report

The distance at which terrain sharpens and the distance at which it coarsens
again don't match. Confirmed: they don't, and it's by design, but the
design's own justification is stale.

## Root cause

`apps/web/src/render/lod.ts`'s `calculate_lod_step` applies two different
hysteresis margins depending on whether a threshold is "blend-eligible":

```ts
const BLEND_ELIGIBLE_THRESHOLDS: u32 = 3; // only thresholds 192/384/768 get dither-blend
...
const margin: f64 = i < BLEND_ELIGIBLE_THRESHOLDS ? (1 - BLEND_BAND_FRACTION) : (1 / LOD_MARGIN);
```

- Thresholds 192/384/768 (`i < 3`): margin `0.95` → a **5% dead zone**, masked
  by a dither-blend so the transition doesn't read as a hard pop.
- Thresholds 1536/3072/6144 (`i >= 3`): margin `1/1.4 ≈ 0.714` → a **28.6% dead
  zone** (≈439 units wide at 1536), with no blend — a hard, and much later,
  snap back to finer detail.

The asymmetry itself is intentional and documented — hysteresis only resists
*becoming finer*, there's no symmetric cushion against becoming coarser. What's
stale is the reasoning for **why 1536 was left in the wide/unblended group**:

> Only thresholds inside the current visible range get dither-blended. The
> third handoff moved from 576 to 768 and is now the last visible transition
> before FOG_FAR=900, so including it removes both its hard pop and the old
> 29% non-blended hysteresis gap. Later thresholds start past the fog range
> and remain hard transitions to avoid retaining unused secondary meshes if
> the render distance is expanded later.

That comment assumes `FOG_FAR = 900`. Earlier in this same session, `FOG_FAR`
was doubled (`apps/web/src/modules/world/environment/config.ts`, now `900 * 2
= 1800`) as part of an unrelated camera/render-distance pass. Threshold 1536
is now well inside the visible range the comment says it sits past, so the
28.6% dead zone it was fine to leave hard now happens during ordinary camera
movement instead of beyond the fog line.

`apps/web/src/render/lod.test.ts` still encodes the old premise — one test
checks behavior at `dist=900` under the name "current fog range," another
checks `dist=1530` under "thresholds past the visible range remain unblended."
Both still pass (they're pure-function tests independent of live config), so
nothing failed; they just no longer test what their names say.

## Does this cost render performance?

The user's hypothesis, asked as a follow-up: does the dead zone itself add
meaningful render load? Code-level analysis says **no, and if anything the
opposite** — this reads as a visual-consistency issue, not a performance one.
Not measured live; see caveat below.

- **Steady state in the dead zone is cheaper, not more expensive.** While a
  chunk sits in the dead zone it renders at the coarser step (fewer instances
  per chunk — a step-16 chunk has 1/4 the instances of step-8). The dead zone
  trades sharpness for cost, in the direction that saves render load, not the
  direction that spends it.
- **The real performance risk in an LOD system is oscillation** — a chunk
  repeatedly crossing a boundary and re-triggering a mesh rebuild each time.
  `chunk_stage.tsx` already budgets against this (`LOD_WORK_BUDGET_MS`,
  `LOD_MAX_CHUNKS_PER_FRAME`, a `bucket_cache_ref` whose own comment names
  "LOD hunting back and forth around a threshold" as the thing it exists to
  avoid) — so this is a known, previously-addressed category of concern in
  this codebase, not a new one.
- **A wider margin makes that oscillation harder, not easier.** Once a chunk
  coarsens at 1536, the camera has to close 439 units before it's allowed to
  sharpen again — a small back-and-forth near the boundary doesn't re-trigger
  a rebuild. Narrowing the margin (the fix that would restore visual
  consistency with the blend-eligible thresholds) makes the boundary easier to
  cross both ways, which trades some of that oscillation resistance away.

So the two axes pull in opposite directions: fixing the visual inconsistency
by tightening the 1536 margin to match the blend-eligible thresholds would
likely trade a small amount of oscillation resistance for it. Not obviously a
bad trade, just not a free one — worth knowing before changing it.

**Caveat:** this is a conclusion from reading `lod.ts` and `chunk_stage.tsx`,
not from profiling a running session. Confirming it would mean panning the
camera to hold chunk distances in the 1097–1536 band and watching whether
`chunk_stage.tsx`'s scan/build path fires repeatedly, per AGENTS.md's own
"verify claims against a running system" convention.

## Options considered, not yet chosen

1. Extend `BLEND_ELIGIBLE_THRESHOLDS` from 3 to 4 so 1536 gets the same 5%
   margin and dither-blend as 192/384/768. Fixes the visual inconsistency
   directly; costs one more blended mesh in memory/build work whenever a
   chunk is near that boundary.
2. Decouple the margin from blend-eligibility — tighten 1536's hysteresis
   margin to ~0.95 without adding a blended mesh for it. Cheaper than (1), but
   means a plain hard snap at a tighter distance rather than a masked one.

Neither has been implemented. Nothing in `render/lod.ts` or
`chunk_stage.tsx` has changed as a result of this investigation.
