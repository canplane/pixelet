# What the strings call things

The words a user reads, in every locale. The code has its own names — `cube`,
`layer/object` — and they are not these; a string is written for someone who
has never seen the code.

What the world holds is named on two axes: where a thing came from, and what it
is drawn as.

|             | **cube** — a box                | **decor** — a model             | **marker** — a flat design |
| ----------- | ------------------------------- | ------------------------------- | -------------------------- |
| **terrain** | the ground, water and buildings | trees and vehicles it generates | the places it names        |
| **object**  | cubes you placed                | decor you placed                | markers you put up         |

| en      | ko       | ja           | zh-Hans | zh-Hant | means                                                           |
| ------- | -------- | ------------ | ------- | ------- | --------------------------------------------------------------- |
| terrain | 지형     | 地形         | 地形    | 地形    | what came from the real world's data                            |
| object  | 오브젝트 | オブジェクト | 物体    | 物件    | what a user placed, of any kind                                 |
| cube    | 큐브     | キューブ     | 方块    | 方塊    | the box the ground and placed cubes are drawn as                |
| decor   | 데코     | デコ         | 装饰    | 裝飾    | a model drawn from a glTF, on the terrain or placed             |
| marker  | 마커     | マーカー     | 标记    | 標記    | a design drawn flat at a place: a place's name, or a pin put up |

Chinese is two catalogs by script, Taiwan's words in Traditional, and a system
tag reads the one its script or region names (`i18n.ts`). A division's or a
place's name is its own in every locale.

An item — each cube, decor, marker and pet — has one name, the English one, in
every locale, so it is no locale's string: it is written beside the item in its
spec (`label` in `spec/spec/asset/cube.json5` and `decor.json5`, `PIN_LABEL` in
`marker.json5`, a pet's `label` in `character.json5`) and read by the item's
number.

A string only a development build shows, a dev setting's among them, is in
`dev/<locale>.json`, merged over its locale in a development build alone
(`i18n.ts`).
