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
