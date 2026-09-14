# Characters (the Actor tier)

This directory is the current, only member of the "Actor" rendering tier —
see [issue/20260907-world-rendering-tiers.md](../../../../../../issue/20260907-world-rendering-tiers.md)
for the full taxonomy this belongs to. Kept as `character/`, not renamed to
`actors/`, on purpose: "Character" is already a real user-facing concept
(Settings > Character, drawer_modal.tsx's Character tab — which variant a
person/pet uses), and renaming the folder to the engine's own tier name would
make it read as a different thing from that settings surface when it's the
same one.

**What makes something an Actor, not one of the other tiers:** one of the
module's actors (crates/pxl-core/src/model.rs) — a model played by its own
clips, its joint palette written each tick — rather than an instance of a
static model the way decor's population is, with position and animation
driven imperatively in a step of the frame's tick (`sched/tick.ts`, the
interaction phase) via refs rather than React state/props, at "a few,
individually distinct" instance count. `companion.tsx`'s `Walker` (person + pet) is the only
instance today.

`chat_bubble.tsx` lives here too — it's not itself an Actor (it's a
world-anchored billboard, a different tier), but its position is entirely
derived from riding its parent Actor's own transform, so it stays next to the
thing it's attached to rather than moving to a general billboard directory.
