# Characters (the Actor tier)

This directory is the current, only member of the "Actor" rendering tier —
see [doc/minutes/20260907-world-rendering-tiers.md](/doc/minutes/20260907-world-rendering-tiers.md)
for the full taxonomy this belongs to. Kept as `character/`, not renamed to
`actors/`, on purpose: "Character" is already a real user-facing concept
(Settings > Character, the drawer's Character tab — which variant a person or
pet uses), and renaming the folder to the engine's own tier name would make it
read as a different thing from that settings surface when it's the same one.

**What makes something an Actor, not one of the other tiers:** one of the
kernel's actors (`/crates/client/kernel/src/scene/actor.rs`) — a model played by
its own clips, its joint palette written each tick — rather than an instance
of a static model the way decor's population is, with position and animation
driven in a step of the frame's tick (`../../sched/tick.rs`, the interaction
phase) rather than by anything the shell holds, at "a few, individually
distinct" instance count. `companion.rs`'s `Figure` (person + pet) is the
only instance today.

The person's chat line is no Actor of its own: it is a billboard label as a
marker's is, laid out with them (`../place.rs`), and the companion
only hands down where it is said from (`../../billboard/chat.rs` `speak_from`).
