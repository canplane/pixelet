# Markers (the one-off indicator tier)

This directory is the current home of the "one-off indicator" rendering
tier — see [issue/20260907-world-rendering-tiers.md](../../../../../../issue/20260907-world-rendering-tiers.md)
for the full taxonomy. `hover_cursor.tsx` and `location_puck.tsx` were
already co-located here before that document named the pattern; this file
just makes the grouping explicit.

**What makes something belong here, not one of the other tiers:** a
singular, ephemeral, bespoke-geometry marker placed in the local world
frame (`frame/coord.ts`), tied to input or device state rather than
persisted world data — and, the trait that actually separates this tier
from tier 3 (world-anchored billboards), **not** camera-facing. `HoverCursor`'s ghost box
and `LocationPuck`'s zone ring/beacon both sit and rotate in real world
space; neither counter-rotates to face the camera the way a billboard would.
`style.ts`'s `marker_colors` is shared styling for this tier specifically
(`ACTION`, `VOID`), not a general color constant file.

`debug_gizmos.tsx` lives here too but is a `__IS_DEV__`-only debug
visualization (frame axes, rings and labels, as the module's line segments
and DOM labels), not a real one-off indicator in the same sense — adjacent
by folder convenience, not by tier.
