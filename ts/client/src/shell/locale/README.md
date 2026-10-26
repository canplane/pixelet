# What the strings call things

The words a user reads, one meaning each, in every locale. The code has its own
names — `cube`, `layer/object`, the pyramid's pages — and they are not these; a
string is written for someone who has never seen the code.

What the world holds is named on two axes: where a thing came from, and what it
is drawn as.

|             | **cube** — a box                | **decor** — a model         | **marker** — a flat design |
| ----------- | ------------------------------- | --------------------------- | -------------------------- |
| **terrain** | the ground, water and buildings | trees and vehicles it grows | the places it names        |
| **object**  | cubes you placed                | decor you placed            | markers you put up         |

| en        | ko       | means                                                            |
| --------- | -------- | ---------------------------------------------------------------- |
| world     | 세계     | the space that is drawn and built in, all of it                  |
| terrain   | 지형     | what came from the real world's data                             |
| object    | 오브젝트 | what a user placed, of any kind                                  |
| block     | 블록     | anything that stands in the world: a cube, a decor or a marker   |
| cube      | 큐브     | the box the ground and placed cubes are drawn as                 |
| decor     | 데코     | a model drawn from a glTF, on the terrain or placed              |
| marker    | 마커     | a design drawn flat at a place: one the map has, or a pin put up |
| character | 캐릭터   | the person and the pet                                           |
| ground    | 땅       | the surface something stands on, as a plain word                 |

Not used: map, voxel, pixel, and world for anything but the world above or the
real one ("현실 세계"). A setting's cost is said in words a user has — light on
performance, the most demanding option — not in operations per pixel.

An item — each cube, decor, marker and pet, the `blocks.*` and `pets.*` keys —
has one name, the English one, in every locale: only en.json holds it, and every
other locale falls back to it. A section's header is written in the reader's
language.
