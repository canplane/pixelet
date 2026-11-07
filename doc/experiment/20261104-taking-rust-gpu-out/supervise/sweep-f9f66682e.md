# Sweep of world.wgsl at f9f66682e

443 operator mutants (records/f9f66682e-sweep.txt, results in f9f66682e-sweep.out): 388 fail the test, 7 hang it, 48 survive. Each survivor is sorted by reading, not by proof. The text of line 457 is since deleted from the working tree.

Run: tools/round.sh copies the tree to a scratch directory and adds the mutant loop (tools/patch.py) to the copy of tests/world.rs; tools/gen_sweep.py makes the mutants; tools/run_par.py runs each in its own process, 60 s a mutant. tools/livepath.py counts live words along a call path. tools/tint.mjs compiles a module through Tint and Dawn. The scratchpad the tools name (`SP`) was the session's that ran them, since deleted, and tools/tint.mjs resolves `webgpu` from `ts/app/web`, the page's tree of the time.

## reachable, the test does not reach it (3)

- L68:38: `let l = min(lo, vec3<u32>(BOX_STEPS - 2u));`
- L610:47: `let lo = u32(clamp(first, 0i, i32(BOX_STEPS) - 2i));`
- L774:54: `let part = (scaled >> 16u) + u32((scaled & 0xffffu) >= (w & 0xffffu));`

## needs a height at an i16 end (4)

- L457:10: `if (b.x != UNBOUNDED_LO && b.y == UNBOUNDED_HI) {`
- L457:33: `if (b.x == UNBOUNDED_LO && b.y != UNBOUNDED_HI) {`
- L457:26: `if (b.x == UNBOUNDED_LO || b.y == UNBOUNDED_HI) {`
- L498:29: `if ((b.x <= UNBOUNDED_LO || b.y >= UNBOUNDED_HI) || (got.clip <= bottom && got.all)) {`

## holds only by what callers or geometry give (6)

- L69:59: `let h = select(min(hi, vec3<u32>(BOX_STEPS)), l + 1u, hi < l) - 1u;`
- L69:50: `let h = select(min(hi, vec3<u32>(BOX_STEPS)), l + 2u, hi <= l) - 1u;`
- L93:33: `return vec3<u32>(w.x & m, (w.x << (2u * BOX_BITS)) & m, (w.y >> BOX_BITS) & m);`
- L617:14: `if (bound.x > bound.y) {`
- L971:64: `if (k < 5u && blocks == 0u && ((k == 4u && last > 0u) || (k <= 4u && tunnels[k] > 0u))) {`
- L1027:8: `if (t < 0i) {`

## reads past the table: 0 on Metal, Dawn clamps (1)

- L389:24: `var stop = FIRST_CELL - 2u;`

## equal outright (34)

- L58:52: `return Cover(select(under_fill - taken, 0u, taken >= under_fill), false);`
- L140:70: `return select(i32(1u << ((zoom - SHARD_ZOOM - 1u) & 31u)), 0i, zoom < SHARD_ZOOM);`
- L209:36: `return select(0u, four_of(v), v <= 256u);`
- L223:10: `if (c > 256u) {`
- L243:39: `if (slot_at(PLACED, base + k, zoom + 2u) != SLOT_NOTHING) {`
- L267:70: `return (word >> 16u) + 2u > select(0u, FULL >> (shift & 31u), shift < FULL_BITS);`
- L285:17: `if (root == 0u && zoom < top) {`
- L450:42: `let at = vec2<u32>(q & vec2<i32>(across - 2i)) >> vec2<u32>(level);`
- L466:96: `let g = select(c >> vec2<u32>((zoom - cell) & 31u), c << vec2<u32>((cell - zoom) & 31u), cell > zoom);`
- L475:39: `let j = select(0u, clip - zoom, zoom <= clip);`
- L548:88: `let part = select(rest >> ((n - FULL_BITS) & 31u), rest << ((FULL_BITS - n) & 31u), n < FULL_BITS);`
- L563:59: `return select(1i, 1i << ((cell - LEAF_CELL) & 31u), cell > LEAF_CELL);`
- L585:52: `return first < (span.y << e) && first + (1i << s) >= (span.x << e);`
- L591:102: `let scaled = select(first << ((BOX_BITS - shift) & 31u), first >> ((shift - BOX_BITS) & 31u), shift > BOX_BITS);`
- L606:88: `let past = select(z_step(span.y, z, shift), z_step(span.y - 1i, z, shift) + 1i, shift > BOX_BITS);`
- L775:39: `let least = select(0u, n - room_1, n >= room_1);`
- L776:11: `if (part <= least) {`
- L816:34: `select(mx - dx, mx + dx, mx <= 2i * q),`
- L817:34: `select(my - dy, my + dy, my <= 2i * q),`
- L843:21: `for (var j = 0u; j <= 3u && flipped < i; j += 1u) {`
- L912:21: `for (var i = 0u; i <= n && i <= at; i += 1u) {`
- L912:25: `for (var i = 0u; i < n || i <= at; i += 1u) {`
- L936:37: `for (var axis = 0u; axis < 3u && n >= 0u; axis += 1u) {`
- L968:11: `while (z < CAVE_PIPE) {`
- L971:10: `if (k <= 5u && blocks == 0u && ((k == 4u && last > 0u) || (k < 4u && tunnels[k] > 0u))) {`
- L1029:15: `} else if (t > l) {`
- L1041:13: `if (blocks > CELL_BLOCKS) {`
- L1049:45: `for (var o = 0u; o < CELL_BLOCKS && before <= blocks; o += 1u) {`
- L1060:11: `if (zoom < CAVE_PIPE) {`
- L1071:11: `if (zoom > CAVE_BLOCK) {`
- L1091:11: `if (zoom >= LEAF_CELL) {`
- L1096:11: `if (zoom < CAVE_ROOT) {`
- L1103:12: `if (shift > 0i) {`
- L1174:26: `if (g.kind == KIND_CELL || lookup_here(g.place)) {`
