# What the strings call things

The words a user reads, one meaning each, in every locale. The code has its
own names — `cube_type`, `layer/object`, the pyramid's pages — and they are
not these; a string is written for someone who has never seen the code.

What the world holds is named on two axes: where a thing came from, and what
it is made of.

|             | **block** — a voxel             | **decor** — a model, not voxels |
| ----------- | ------------------------------- | ------------------------------- |
| **terrain** | the ground, water and buildings | trees and vehicles placed on it |
| **object**  | blocks you placed               | decor you placed                |

| en        | ko       | means                                                                               |
| --------- | -------- | ----------------------------------------------------------------------------------- |
| world     | 세계     | the space that is drawn and built in, all of it                                     |
| terrain   | 지형     | what came from the real world's data                                                |
| object    | 오브젝트 | what a user placed, blocks or decor                                                 |
| block     | 블록     | the voxel both are made of; a block, not a cube, since a block need not stay a cube |
| decor     | 데코     | a model drawn from a glTF, on the terrain or placed                                 |
| character | 캐릭터   | the person and the pet                                                              |
| ground    | 땅       | the surface something stands on, as a plain word                                    |

Not used: map, cube, voxel, pixel, and world for anything but the world above or
the real one ("현실 세계"). A setting's cost is said in words a user has —
light on performance, the most demanding option — not in operations per
pixel.
