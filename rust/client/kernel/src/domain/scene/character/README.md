# Characters (the Actor tier)

An **actor** is a model that plays its own clips: each tick advances its
clips and writes its joint palette into its block (`../actor.rs` `Actors`).
That is what sets this tier apart from decor, whose population is instances
of static models, and from the indicators, which have no clips. Actors are
few and each is distinct. This directory holds every one of them:

- the companion, the person and the pet at the focus (`companion.rs`
  `Figure`), placed each tick in the interaction phase
  (`../../../turn.rs` `companion_tick`);
- the variant picker's display cases, a character idling in its case's own
  canvas (`preview.rs`).

Where an actor stands is the kernel's, never the shell's. Its clips advance
in the scene's own tick (`../../scene.rs`).

The person's chat line is not an actor. It is a marker hung from the
point over the person's head that the companion hands down (`speak_from`),
shown by its own rule (`../marker/chat.rs` `hangs`) and laid out with
the others (`../marker/arrangement.rs`).

The folder is `character/`, not `actors/`, because "Character" is already
what a user reads: the settings' Character section and the drawer's
Character tab choose which variant the person and the pet use. The tier's
place among the others is
[doc/minutes/archive/20260907-world-rendering-tiers.md](/doc/minutes/archive/20260907-world-rendering-tiers.md).
