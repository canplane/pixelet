# Third-party notices

Some of what is here is a copy of someone else's work rather than our own;
this records where each came from and under what license. One of them, Arnis,
asks for a notice to travel with the copy. xpute, the system pixelet runs on,
is developed in this repository and keeps its own record of what it ported:
[xpute/THIRD_PARTY_NOTICES.md](./xpute/THIRD_PARTY_NOTICES.md).

This file covers what is copied into the tree: source, a font, drawings and
colors. Crates and npm packages that are merely depended on are not listed:
`rust/Cargo.toml` and the `package.json` files name them, and each carries its
own license where it is published. The models and textures under `asset/` are
recorded there, each pack with its license beside it
([asset/README.md](./asset/README.md)).

Data sources have their own obligations and their own place:
[infra/README.md](./infra/README.md) §6 records them, and
`ts/client/src/ui/feature/setting/info/attribution.tsx` is where the ones that must
reach a user are shown.

## louis-e/arnis — `rust/server/geo-bake/src/separation/solve.rs` and `solve/`, `rust/spec/src/procedural.rs`

How bridges and tunnels stand is [louis-e/arnis][arnis] at `c96872e`
(Apache-2.0), its [element processing][processing] for bridges, highways and
railways without the blocks it places: the bake's solve in
`separation/solve.rs`, its bridges in `solve/bridge.rs` and its tunnels in
`solve/tunnel.rs`, each of whose heads carries the license notice, and the piers
in `procedural.rs`, which reach the client; the parts taken from it are marked
where they begin (`param.rs`, `way.rs`). Both read Overture's fields for OSM's
tags and take their widths from the ground.
Arnis's `NOTICE` asks that this be kept with them:

```text
Arnis
Copyright (c) 2022-2026 Louis Erbkamm (louis-e)

This product is developed and maintained by Louis Erbkamm (louis-e).
It is free and open source software, licensed under the Apache License,
Version 2.0. See the LICENSE file for the full terms.

The only official sources for Arnis are:
  https://github.com/louis-e/arnis
  https://arnismc.com

If you redistribute this software, keep the attribution notices in this file
intact. This is required by section 4 of the Apache License, Version 2.0.
```

The rest of that file is about the data and the assets Arnis bundles, none of
which is taken. The license's text is
[LICENSES/Apache-2.0.txt](./LICENSES/Apache-2.0.txt), and the client shows the
notice under Settings, Attribution
(`ts/client/src/ui/feature/setting/info/attribution.tsx`).

## N8python/n8ao — `rust/client/render/src/frame/shader/ao_view.wgsl`, `pass/blue_noise.bin`

The screen-space ambient occlusion is [N8AO][n8ao] 1.10.1 (CC0 1.0), ported to
WGSL and to reversed depth, and the blue noise it reads is N8AO's
(`rust/client/render/src/pass/ao.rs`). CC0 asks for no notice.

## mapbox/maki — `asset/raw/place/symbol/`

The place icons are Mapbox's [Maki][maki] 8.2.0 (CC0 1.0), drawn into the code
that carries them. CC0 asks for no notice.

## carbon-design-system/carbon — `spec/spec/bake/place_theme.json5`, `rust/client/render/src/look/marker.rs`

The places' colors and the markers' dark gray are IBM's [Carbon][carbon] color
tokens, `@carbon/colors` 11.62.0 (Apache-2.0), copied as values. The license's
text is [LICENSES/Apache-2.0.txt](./LICENSES/Apache-2.0.txt).

## gravitystorm/openstreetmap-carto — `spec/spec/asset/color.json5`, `cube.json5`, `spec/spec/bake/place_theme.json5`

The ground's palette and paint order, and the places' categories and zooms,
follow [OpenStreetMap Carto][carto] (CC0 1.0). CC0 asks for no notice.

## u_DIN 1451 Mittelschrift — `ts/client/public/font/`

The font is u_DIN 1451 Mittelschrift, Copyright (c) 2016 usr_share and
Copyright (c) 2009-2016 Peter Wiegel, under the SIL Open Font License 1.1. Its
license is served beside it (`uDIN1451Mittelschrift-OFL.txt`).

[arnis]: https://github.com/louis-e/arnis
[n8ao]: https://github.com/N8python/n8ao
[carto]: https://github.com/gravitystorm/openstreetmap-carto
[maki]: https://github.com/mapbox/maki
[carbon]: https://github.com/carbon-design-system/carbon
[processing]: https://github.com/louis-e/arnis/tree/c96872e0fa21573402d1b731ea87f796e8caa1d2/src/element_processing
