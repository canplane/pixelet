# One number, and what clears entropy

**2026-11-03** — recorded with the user, after the commits from `f59ad6301`.

## What the user decided

- **This is the model of clearing state.** Agents are to know what clearing entropy
  is by it (the `facts` skill, "Clearing entropy").
- **A cube and a decor are their `AssetId`.** `DecorId` and a `Cube` type were the same
  number. The user asked whether it could simply be called an id.
- **`Cube` is the tool's name** for `ActiveTool`'s `Object`, which survived from August:
  every tool touches the object layer, so it told nothing apart. `Cube::Object` went
  with the type; the catalog's `object` stays, what only placing makes, as in
  Minecraft, of which today only colors exist.
- **An id the catalog does not give out falls back to unknown**, one rule for every
  kind.
- **`Asset` holds behavior, not data.** What each kind does is its methods; the user
  named the repeated "which cube, which decor" a matter of polymorphism. A kind's look
  and its table row stay outside, as a file's data does from its metadata.
- **A size is a scale of the base block, not a zoom.** A cube's is chosen, in the
  popover; a decor's and a marker's are not. "Zoom" is what it comes to, and the tool
  holds the scale: `scale_steps`, 0, -1, -2 and -3, as a decor's already was. `tier` is
  gone.
- **No migration of the hotbar's stored form.** It fills back in.

## What was built

`Asset` (`domain/layer/object/asset.rs`): `Asset::of` is the one reading of an id, and
`zoom`, `room` and `place` are its own, beside the tool where they read the world.
`Tool::asset` says what a tool would place; `Placeable` is gone. The ghost settles what
it shows once. `Hover` keeps one `target`. The shell's hotbar is a list of ids;
`selectAsset` and `useInHand` take one, and `toolOf` reads its kind from `id.spec.ts`,
now generated for TypeScript too.

Three enums name the kinds, each saying something else: `Kind` the number's bits,
`ActiveTool` the protocol's tools, `Asset` the catalog's reading.

- **The dock's state is the shell's.** The kernel has no reason to keep it or say it
  back. It is told what the pointer holds, and answers an errno, negative, which the
  shell checks before keeping what it chose.

The protocol's `Tool` (`ActiveTool` before; the user took the name, and `Item` for what is
held: an asset may be an actor or the sky, which nothing holds) is the pointer, the eraser
or an item, whose id says its kind. In the kernel `Tool` is an enum: only an item has a
scale, and it keeps no size to remember.
The shell's dock store keeps the tool, the id and the scale, and the snapshot gives only
what the kernel makes, the draft.

## Who says a file's name, and whose word "host" is

The user asked whether the object store holds two facts. It does not: **the
TypeScript source is a mock in a server's place, the user said**, and its copy of the
tree's rules is a mock's (`object_tree.ts`). The bakes have one set of rules because
baker and kernel link one crate, and the driver reads none of a tile.

`tile_name` was another matter. The kernel, which links the rule, handed a number, and
the driver, which does not, read its bits back into a name, a port held by a fixture.
**The user decided this is a question of responsibility: the kernel says the name.** A
tile and a shard open by their quadkey (`Tile::quadkey`), the sphere, the top and the
edits by their own names (`publication.json` `SPHERE`, `file.json` `object`), a load and
a slot by their id in decimal. `number` goes from these, the user having said it reads
oddly. The driver puts a version before a name and knows nothing of its bits; the mock
keeps its own `quadkey`, to file a write by its shard. The contract changed on purpose:
a path below a root was a number and is now a name. The DB's keys and the bucket's
objects are what they were.

**The host is xpute**, the user said: it carries, and what answers the kernel's opens is
a driver in TypeScript, as if another guest. The word had spread to what drivers do, and
a session read it and reasoned from it. So `driver`, `client` or `CPU` now stands where
it was wrong, and `host.json` holds the hostcalls alone, with the roots and the files in
`file.json`. Three shaders' comments still say it (`screen_propagate.wgsl`,
`tile_mark.wgsl`).

## One key, written one way at the edge

The user read the key's shape and asked why it was so. A tile and an object are both
morton, 2D or 3D, so **the user decided they cross the kernel's edge one way, and the edge
is text**: a path is text in xpute's `OPENAT` and a bucket's name is too, so a tile and a
shard go by their quadkey and a leaf by its octkey, the same digits to z8 and one digit a
zoom past it. `write` and `top` hold them as `strs` (formats 19 to 21). No TypeScript reads
a key's bits. The `Z` column stays, the key's next bit, Z's sign, which a digit cannot
write alone.

Inside the kernel a key is bits. **A tile's is a `u64`**: `u128` was only ever
for a 3D address, 74 bits at z27. `subject` was the high half of a tile's `u128`, and the
queue's one word beside an asset's: it is gone, and the queue's word is an `id`, its
meaning its stream's. The profiler's task row holds the path it opened.

**The key's top bit goes, the user said**: no record or code said why it was left 0.

The names follow, the user's: a node of the plane is a `Quadrant` (`Tile` before, its one
meaning geometry), one under a shard an `Octant` (`EntryAt`), and a tile stays what is
fetched and stored as one. **`key` is the text, the quadkey or the octkey, and `id` the number,
its morton code**: a number named `key` said nothing, and most of its uses packed a node to
unpack it later. A `Write` holds its address as `at`, its `id` being the asset's. What the
queue's `id` holds is still its stream's to say; a typed want is open.
