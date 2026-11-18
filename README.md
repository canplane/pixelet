# pixelet

**[▶ Play](https://plei.me)**

> _A droplet fades. A pixelet remains._

_Started on December 15, 2025._

**pixelet puts the real Earth into cubes — your street, its buildings, the river
beside it, the hills behind it — and lets you build on it.** It runs in a
browser tab and as an app on iOS and Android.

<img alt="Voxel objects built on real-world terrain" src=".github/asset/screenshot.webp" width="100%">

## How a planet fits in a phone

Cut the Earth into cubes 2.4 meters wide and there are 16.8 million of them
around the equator, and as many again north to south, before you count a single
one of height. No disk holds that, no network sends it and no GPU draws it.
pixelet gets away with it because a world only has to exist as far as someone is
looking.

A screen is finite, so what is seen is sampled. pixelet divides the view into
cells of equal angle, and looks at every place at the size that fills one of
them: 2.4-meter cubes at your feet, cubes twice as large twice as far away, and
so on out to the horizon.

```text
  the eye ●───────────┬───────────────────────┬─────────────────────── ▶
            2.4 m cubes       4.8 m cubes             9.6 m cubes
                       twice as far            twice as far again
```

What a frame asks of the world therefore depends on how many cells the screen
has, not on how big the world is. A city costs what it covers on your screen.
A change too small to tell apart is no change either, so a still view costs
almost nothing.

That turns the world from data into a question: _what fills this cube of space,
at this size?_ The ray that draws a pixel, the feet of a walking character, the
pointer and the hand that places a block all ask one function, so what you see
is what you stand on. The answer is always a number that names what fills the
cube, the same whether the ground put it there or a player did. And there is an
answer at every size: the ground is baked into tiles that hold that number for
each cell and the elevation it is worked out from, folded four into one for
each coarser size; trees and caves grow from one seed wherever someone asks
about them; only what players build is stored. What is not ready yet is
never made up: a coarser tile answers until the finer one is ready, as the last
picture does until the next, so the picture only ever gets sharper.

```text
  a pixel's ray       a body's step       the pointer      an edit
   (the GPU)           (the CPU)          (the CPU)       (the CPU)
       └──────────────────┴──────────┬──────┴───────────────┘
                                     ▼
                         what fills this cube?
                                     │
                  what players placed              ┐ each written
                  the ways off the ground          │ over the one
                  the ground and its caves         ┘ below it
```

The world lives in one Rust core that runs the same in a browser and on a phone,
drawing with WebGPU, on [xpute](./xpute/README.md), a small system written for
it. The geography comes from [Overture Maps](https://overturemaps.org/),
OpenStreetMap's coastline and the
[GEDTM30](https://doi.org/10.5281/zenodo.18887460) elevation model, baked into
tiles ahead of time and served as static files.

## Running it

With GNU Make, Python, Node with pnpm, Rust and Binaryen installed:

```bash
make install
make dev
```

`make dev` serves the page and takes the tiles from the live site.
[CONTRIBUTING.md](./CONTRIBUTING.md) has the rest: the local servers, the native
builds and the checks.

## Learn more

[ARCHITECTURE.md](./ARCHITECTURE.md) follows the idea through the system, from a
cube of space to a frame on the screen, and ends with a map of the code.

---

[github.com/canplane/pixelet](https://github.com/canplane/pixelet)
