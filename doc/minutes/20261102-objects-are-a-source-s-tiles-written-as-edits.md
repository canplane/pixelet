# The objects are a source's tiles, written as edits

**2026-11-02** — recorded with the user. Main was at `e42d5dc37f` when it
began; the first step is `fec96e2857` (objects a shard at a time through the
`OBJECT` root) and `c911fe7cdc` (their place in the serve order); the second,
edits up and files down, is being built. The commits are named as `original`
keeps them.

## What was asked

Every world read the kernel makes goes through one interface — a root's number
and an object's number below it, bound by the driver (`sys/io/file.rs`,
`ts/client/src/driver/namespace/`) — except the placed objects, which were one
blob in a storage file beside the settings, read whole at boot. The user asked
whether everything else was unified, and then for the objects to be unified
too.

## What the user decided

- **One repository layer, whatever is behind it.** The kernel's object
  repository (`domain/layer/object/repository.rs`) reads and writes through the
  same interface as the baked tiles; whether a loopback, the device's storage or
  a network is behind the root, the kernel cannot tell. The same API shape
  everywhere, code merged where the roles are the same: "웬만해선 같은 API
  형식을 쓰고, 코드를 합치는게 좋아보여. 그렇다고 무작정 다 합치는걸 원하는건
  아니고."
- **A shard a file**, the world's unit (`SHARD_ZOOM`, ARCHITECTURE.md); a shard
  with no objects has no file.
- **No migration**: the old blob (`pxl:kept:objects`) is neither read nor moved
  ("NO Migration. 신경쓰지마.").
- **The serve order**: the objects' shards ride the ground's order by shell —
  seen ground, seen objects, unseen ground, unseen objects — then the assets,
  the places and the Earth, the divisions. This continues 2026-09-26's order
  (terrain, the assets decor draws with, the places; placed objects riding the
  ground), the user leaving the detail to judgment ("순서 적당히 재조정할 거는
  재조정해").
- **The object tree is edited collaboratively, so the line drawn now is the
  interface collaboration needs, and nothing more** (the user: "일단은 이렇게
  해두는게 보수적인 최소한의 안정적 선이다?"):
  - writes go up as edits, the repository's `<key, id, ext>` batches, never as
    files;
  - reads come down as files the source keeps: each shard's, and the top of
    the tree above `SHARD_ZOOM`, what each shard folds to from afar and so
    which shards hold objects, from which a zoomed-out view draws;
  - the top is derived, owned by the source, which rebuilds it as edits land;
    the kernel never writes it;
  - the device's storage is the loopback today, applying edits and keeping
    both kinds of file, so a server can take its place with the kernel
    unchanged.

## What was weighed and not taken

- **The client writing whole shard files**, as the first step did: with two
  editors, the later write erases the earlier. Kept only until the edits path
  lands.
- **A list file the client keeps** for which shards hold objects: one file every
  write anywhere touches, and derived data with the client as its owner.
- **A summary file at every zoom**, as the terrain's pyramid is baked: what the
  tree's top already is, at the cost of a file per level rewritten on each
  write.

## Open

- A server, and what it decides: the order of concurrent edits, their
  conflicts (a CRDT or otherwise), who may write.
- Letting go of shards read, and a network or loopback backend beside the
  device's.
- A shard file the kernel refuses is not reported or set aside, as a refused
  settings file is.
- One fact computed in several places: the shard over a node (the repository,
  `sys/io/source.rs`, the coverage gizmo, geo-bake's `coverage.rs`), and the
  storage's base64 (the kernel for the settings, the host for the objects).
