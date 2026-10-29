# What the strings call things

The words a user reads, in every locale. The code has its own names — `cube`,
`layer/object`, the pyramid's pages — and they are not these; a string is
written for someone who has never seen the code.

What the world holds is named on two axes: where a thing came from, and what it
is drawn as.

|             | **cube** — a box                | **decor** — a model         | **marker** — a flat design |
| ----------- | ------------------------------- | --------------------------- | -------------------------- |
| **terrain** | the ground, water and buildings | trees and vehicles it grows | the places it names        |
| **object**  | cubes you placed                | decor you placed            | markers you put up         |

| en      | ko       | means                                                           |
| ------- | -------- | --------------------------------------------------------------- |
| terrain | 지형     | what came from the real world's data                            |
| object  | 오브젝트 | what a user placed, of any kind                                 |
| cube    | 큐브     | the box the ground and placed cubes are drawn as                |
| decor   | 데코     | a model drawn from a glTF, on the terrain or placed             |
| marker  | 마커     | a design drawn flat at a place: a place's name, or a pin put up |

An item — each cube, decor, marker and pet, the `assets.*` and `pets.*` keys —
has one name, the English one, in every locale: only en.json holds it, and every
other locale falls back to it.

A string only a development build shows, a dev setting's among them, is in
`dev/<locale>.json`, merged over its locale in a development build alone
(`i18n.ts`).
