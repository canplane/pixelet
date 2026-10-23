# The sky was painting over decor, and material.id decided who lost

**2026-09-10** — a bug that survived eight hypotheses, five days of it being
blamed on decor, one restructure aimed at the wrong layer, and a commit that
claimed to have fixed it. The cause was one missing `renderOrder` on the sky
dome, three files away from anything decor owns.

## The symptom, as reported

A tree draws as its own exact silhouette with the sky inside it. Not white,
not untextured, not a missing map — **the sky**, with the clouds in it, and at
night the night sky. The terrain the tree stands on does not show through; the
tree occludes it normally. About one tree in four.

The same thing happens to the location puck's beacon, perhaps once in a couple
of hundred loads. It does not happen to terrain, to characters, or to the
eraser's translucent decor ghost.

Every one of those facts is load-bearing and every one of them was reported by
eye. The investigation went wrong for as long as it did partly because the
report was repeatedly re-read as "the model renders white", which is a
different bug with different causes, and the difference is visible and was
stated plainly more than once.

## What was eliminated, and why none of it helped

In order: a geometry/material arrival race (already guarded); a shared program
cache key dropping `USE_EMISSIVEMAP` (real, measured at `unpatched 3`, fixed —
and not this); a missing texture image (`imageless 0`); fog; missing UVs;
material render state (`colorWrite`, `depthWrite`, `depthTest`, `transparent`,
`opacity`, `side`); mesh flags and winding; orphaned meshes; `visibility_coverage`
at the base LOD tier.

All of those inspect the object. The object was never wrong. Every decor bank
inspects healthy right now — `position,normal,tangent,uv`, a decoded 512×512
`colormap.png`, `onBeforeCompile` fired — and one of them still drew as a hole.
That should have been the signal much earlier: **when everything about an
object is correct and it still does not appear, the object is not the
subject.**

## The mechanism

`environment.tsx` renders the sky as a real mesh — a `sphereGeometry` scaled to
`SKY_DOME_SCALE`, `side: BackSide`, `frustumCulled: false`, with a
`shaderMaterial` at `depthWrite: false` and `depthTest: false`. It had no
`renderOrder`.

three sorts its opaque render list with `painterSortStable`
(`WebGLRenderLists.js`), which compares, in order:

```
groupOrder → renderOrder → material.id → materialVariant → z → object.id
```

Distance is fourth. **`material.id` outranks it** — a global counter
incremented once per `Material` constructed, for the lifetime of the page.

So with no `renderOrder`, the sky dome takes its place in the draw order by
when its material happened to be constructed. Everything constructed before it
draws first. The sky then paints over all of it, because `depthTest: false`
means it never loses a depth comparison — and leaves the depth buffer alone,
because `depthWrite: false`.

That is the whole bug, and it explains each reported fact exactly:

- **The silhouette is perfect** because the depth the tree wrote is still
  there. It is the tree's own geometry, to the pixel.
- **It is filled with sky** because the sky literally painted those pixels.
  Clouds and stars come along because they are in the same shader.
- **The terrain behind it does not show** because the terrain draws later and
  fails the depth test against the tree's surviving depth.
- **No color is ever applied** because the tree's color was written and then
  overwritten in the same frame.
- **Terrain is unaffected** because its material is constructed lazily, on the
  first chunk attach, long after the sky's.
- **The eraser's ghost is unaffected** because `hover_cursor.tsx` builds its
  own material at hover time.

## The numbers, read out of the live scene

```
sky     material.id  48   renderOrder  -1 (after the fix)   depthTest false  depthWrite false
decor   material.id  32, 33, 35        renderOrder 0
beacon  material.id  46                renderOrder 0
chunk   material.id  345               renderOrder 0
```

Decor's materials are built in `decor/asset_store.ts`, whose module body starts
loading every skin at import time — before React has rendered anything. With a
warm browser cache those `load_texture` promises resolve almost immediately, so
the decor materials are among the first in the page. The sky's is built when
`MapEnvironment` mounts. Decor loses.

Which decor variants lose is a race between five independent texture loads and
one React mount, which is why it is "about one in four" rather than all or
nothing, and why it moved around between sessions.

The beacon is the same story with a longer fuse: `LocationPuck` renders its
mesh only once it has ground, normally well after `MapEnvironment`, so it
usually wins — but not always. The corroboration is inside that same component:
the zone box beside the beacon carries `renderOrder={5}` and has never once
been reported doing this.

## The fix

```jsx
<mesh scale={SKY_DOME_SCALE} frustumCulled={false} renderOrder={-1}>
```

There is no negative `renderOrder` anywhere else in the codebase, so `-1` puts
the dome ahead of every other opaque object unconditionally. It is what the
mesh already meant: a background, not a very distant object.

## What this should change about how the next one is chased

**A `renderOrder`-less material that disables depth testing is a latent
ordering bug, not a rendering choice.** `depthTest: false` says "I do not
participate in depth", which only has a defined meaning once draw order is
pinned. Leaving it to `material.id` means leaving it to construction order,
and construction order here is a network race. Anything in this codebase that
turns off depth testing should carry a `renderOrder`; there is one other such
material (`hover_cursor.tsx`'s coplanar case) and it already does.

**The instrumentation answered a question nobody had.** Two counters were
added over the decor materials, they read `unpatched 3` once, that led to a
real but unrelated fix, and they read zero from then on while the symptom
continued. They were kept anyway, on the reasoning that nothing else would
report the class returning. That is instrumentation written on speculation,
and its cost is not the two lines — it is that it made a wrong conclusion look
measured. It has been removed.

**One restructure was aimed at this and missed.** Decor moved from a merged
mesh per (chunk, variant, role) to one `InstancedMesh` per variant
(`decor/field.ts`) on the theory that a defect presenting as one mesh out of a
hundred and forty behaving differently is a defect in lifetime. It was not. The
restructure is worth keeping on its own terms — an instance is sixteen floats
instead of a vertex copy, the draw count went 141 → 22, and a placement change
writes one run instead of remerging a chunk — but it was chosen for a reason
that turned out to be false, and the record should say so.

**The reporter's eyes were the better instrument.** "The sky shows through,
clouds and all" was stated on day one and is, read literally, the entire
diagnosis. Every hour after that went into re-deriving a fact that had already
been handed over.
