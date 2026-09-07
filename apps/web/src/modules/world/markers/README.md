# Markers (the one-off indicator tier)

This directory is the current home of the "one-off indicator" rendering
tier — see [discussions/20260907-world-rendering-tiers.md](../../../../../../discussions/20260907-world-rendering-tiers.md)
for the full taxonomy. `hover_cursor.tsx` and `location_puck.tsx` were
already co-located here before that document named the pattern; this file
just makes the grouping explicit.

**What makes something belong here, not one of the other tiers:** a
singular, ephemeral, bespoke-geometry mesh anchored via `AnchorNode`
(`anchor/node.tsx`), tied to input or device state rather than persisted
world data — and, the trait that actually separates this tier from tier 3
(world-anchored billboards), **not** camera-facing. `HoverCursor`'s ghost box
and `LocationPuck`'s zone ring/beacon both sit and rotate in real world
space; neither counter-rotates to face the camera the way a billboard would.
`style.ts`'s `marker_colors` is shared styling for this tier specifically
(`ACTION`, `VOID`), not a general color constant file.

`frame_gizmo.tsx` lives here too but is a `__IS_DEV__`-only debug
visualization (frame axes/labels via drei's `Html`/`Line`), not a real
one-off indicator in the same sense — adjacent by folder convenience, not
by tier.
