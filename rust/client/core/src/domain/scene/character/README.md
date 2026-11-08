# Characters

A **character** is a person or a pet: a model that plays its own clips, each
tick advancing them and writing its joint palette into its block
(`../character.rs` `Character`). That sets it apart from decor, whose population
is instances of static models, and from the indicators, which have no clips. The
character kind's catalog lists every one a player can be, by its group, the
person or the pet, and its row there (`spec/spec/asset/character.json`).

The two the player chose, a person and a pet, are the **companion**, which keeps
them by their group's number (`companion.rs` `Companion`). Each is the row its
setting names, taken once its model and its skin have landed, and let go only
for another row (`companion_step`). The companion stays while the eye is the
pivot; it is placed nowhere then.

Where a character stands is the core's, never the UI's: its walk stands it each
turn in the interaction phase (`companion.rs` `Walk`, `../../../turn.rs`
`companion_tick`), the pet at its own place beside the person's, neither
anchored to the other.

Each view draws a character by its own eye. The world draws it where its walk
stands it, by the lens's decor tiers (`../../view/lens.rs`), by how much of a
camera cell it spans: posed down to the model's edge, its pose held down to the
impostor's, and none below (`../character.rs` `Character::tick`,
`Character::draw`), and nothing while the eye is the pivot. A character surface,
the character picker's canvas, draws the companion's person or pet as it is,
whole, before an eye of its own, whatever the world's eye sees of it
(`surface.rs`, `Character::surface_draw`).

The person's chat line is not a character. It is a marker the person holds, over
its head, which the person says shows (`companion.rs` `line_hangs`), laid out
with the others (`../marker/arrangement.rs`).
