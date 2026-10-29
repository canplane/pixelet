# Characters (the Actor tier)

An **actor** is a model that plays its own clips: each tick advances its clips
and writes its joint palette into its block (`../actor.rs` `Actor`). That is
what sets this tier apart from decor, whose population is instances of static
models, and from the indicators, which have no clips. Actors are few and each is
distinct.

An actor's instance is held by its holder, which stands it where it is (its
`Stance`) and declares how many it holds. This directory holds every holder:

- the companion, the person and the pet at the pivot, each a figure the walk
  moves (`companion.rs` `Figure`, `FIGURES`), placed each tick in the
  interaction phase (`../../../turn.rs` `companion_tick`);
- the variant picker's display cases, a character idling in its case's own
  canvas (`preview.rs`, `CASES_MAX`).

Where an actor stands is the kernel's, never the shell's. Its clips advance in
the scene's own tick (`../../scene.rs`).

The person's chat line is not an actor. It is a marker the person holds, over
its head, which the person says shows (`companion.rs` `line_hangs`), laid out
with the others (`../marker/arrangement.rs`).

The folder is `character/`, not `actors/`, because "Character" is already what a
user reads: the settings' Character section and the drawer's Character tab
choose which variant the person and the pet use.
