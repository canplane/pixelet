# Indicators (the one-off indicator tier)

This directory is the current home of the "one-off indicator" rendering
tier — see [doc/minutes/20260907-world-rendering-tiers.md](/doc/minutes/20260907-world-rendering-tiers.md)
for the full taxonomy. `hover_cursor.rs` and `location_puck.rs` were
already co-located here before that document named the pattern; this file
just makes the grouping explicit.

**What makes something belong here, not one of the other tiers:** a
singular, ephemeral, bespoke-geometry indicator placed in the local world
frame (`../../view/frame/coord.rs`), tied to input or device state rather than
persisted world data — and, the trait that actually separates this tier
from tier 3 (world-anchored billboards), **not** camera-facing. The hover
cursor's ghost box and the location puck's zone ring and beacon both sit and
rotate in real world space; neither counter-rotates to face the camera the
way a billboard would. pxl-render `look/color.rs`'s `indicator` is shared styling for
this tier specifically (`ACTION`, `ERASE`), not a general color constant
file.

`frame_gizmo.rs` lives here too but is a debug visualization behind the
`world_gizmo` setting (the focus's cell and the frames' rings), as debug
lines, which the gizmo's own faces hide and nothing of the world does — not
a real one-off indicator in the same sense, adjacent by folder convenience,
not by tier.
