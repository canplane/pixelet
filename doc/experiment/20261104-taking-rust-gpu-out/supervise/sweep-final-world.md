# Final sweep of the query

438 mutants of the assembled world_asked module, tests/world.rs: 381 killed, 7 hang, 50 SURVIVED. Sorted by reading, not proof.

## reachable, the test does not reach it (4)

- line 135: `let l = min(lo, vec3<u32>(BOX_STEPS - 2u));`
- line 155: `let m = BOX_STEPS - 2u;`
- line 572: `let lo = u32(clamp(first, 0i, i32(BOX_STEPS) - 2i));`
- line 708: `let part = (scaled >> 16u) + u32((scaled & 0xffffu) >= (w & 0xffffu));`

## needs a height at an i16 end (1)

- line 480: `if ((b.x <= UNBOUNDED_LO || b.y >= UNBOUNDED_HI) || (got.clip <= bottom && got.all)) {`

## holds only by what callers or geometry give (7)

- line 136: `let h = select(min(hi, vec3<u32>(BOX_STEPS)), l + 1u, hi < l) - 1u;`
- line 136: `let h = select(min(hi, vec3<u32>(BOX_STEPS)), l + 2u, hi <= l) - 1u;`
- line 156: `return vec3<u32>(w.x & m, (w.x << (2u * BOX_BITS)) & m, (w.y >> BOX_BITS) & m);`
- line 577: `if (bound.x > bound.y) {`
- line 885: `if (k <= 5u && blocks == 0u && ((k == 4u && last > 0u) || (k < 4u && tunnels[k] > 0u))) {`
- line 885: `if (k < 5u && blocks == 0u && ((k == 4u && last > 0u) || (k <= 4u && tunnels[k] > 0u))) {`
- line 937: `if (t < 0i) {`

## reads past the table: 0 on Metal, Dawn clamps (1)

- line 391: `var stop = FIRST_CELL - 2u;`

## equal outright (37)

- line 129: `return Cover(select(under_fill - taken, 0u, taken >= under_fill), false);`
- line 192: `return select(i32(1u << ((zoom - SHARD_ZOOM - 1u) & 31u)), 0i, zoom < SHARD_ZOOM);`
- line 245: `return select(0u, four_of(v), v <= 256u);`
- line 258: `if (c > 256u) {`
- line 276: `if (slot_at(PLACED, base + k, zoom + 2u) != SLOT_NOTHING) {`
- line 298: `return (word >> 16u) + 2u > select(0u, FULL >> (shift & 31u), shift < FULL_BITS);`
- line 312: `if (root == 0u && zoom < top) {`
- line 440: `let at = vec2<u32>(q & vec2<i32>(across - 2i)) >> vec2<u32>(level);`
- line 450: `let g = select(c >> vec2<u32>((zoom - cell) & 31u), c << vec2<u32>((cell - zoom) & 31u), cell > zoom);`
- line 459: `let j = select(0u, clip - zoom, zoom <= clip);`
- line 515: `if (cell > LEAF_CELL) {`
- line 521: `let part = select(rest >> ((n - FULL_BITS) & 31u), rest << ((FULL_BITS - n) & 31u), n < FULL_BITS);`
- line 528: `if (cell > LEAF_CELL) {`
- line 534: `return select(1i, 1i << ((cell - LEAF_CELL) & 31u), cell > LEAF_CELL);`
- line 551: `return first < (span.y << e) && first + (1i << s) >= (span.x << e);`
- line 555: `let scaled = select(first << ((BOX_BITS - shift) & 31u), first >> ((shift - BOX_BITS) & 31u), shift > BOX_BITS);`
- line 568: `let past = select(z_step(span.y, z, shift), z_step(span.y - 1i, z, shift) + 1i, shift > BOX_BITS);`
- line 646: `if (zoom <= TOP_ZOOM) {`
- line 709: `let least = select(0u, n - room_1, n >= room_1);`
- line 710: `if (part <= least) {`
- line 745: `select(mx - dx, mx + dx, mx <= 2i * q),`
- line 746: `select(my - dy, my + dy, my <= 2i * q),`
- line 769: `for (var j = 0u; j <= 3u && flipped < i; j += 1u) {`
- line 833: `for (var i = 0u; i <= n && i <= at; i += 1u) {`
- line 833: `for (var i = 0u; i < n || i <= at; i += 1u) {`
- line 855: `for (var axis = 0u; axis < 3u && n >= 0u; axis += 1u) {`
- line 882: `while (z < CAVE_PIPE) {`
- line 939: `} else if (t > l) {`
- line 949: `if (blocks > CELL_BLOCKS) {`
- line 957: `for (var o = 0u; o < CELL_BLOCKS && before <= blocks; o += 1u) {`
- line 966: `if (zoom < CAVE_PIPE) {`
- line 977: `if (zoom > CAVE_BLOCK) {`
- line 995: `if (zoom >= LEAF_CELL) {`
- line 1000: `if (zoom < CAVE_ROOT) {`
- line 1006: `if (shift > 0i) {`
- line 1062: `if (g.kind == KIND_CELL || lookup_here(g.place)) {`
- line 1129: `if (a > arrayLength(&asked)) {`
