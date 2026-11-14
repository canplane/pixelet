// pxl-content/cave.rs

//! wgsl-port: the caves, worms of tunnel decided from the top down, so that a node's share
//! of cave is its own before anything under it is asked, and a coarse cell holds exactly what
//! its leaves do.
//!
//! Every root cube (`ROOT`) holds the same tunnels, so every coarser node holds the same
//! share. A tunnel starts in a node of its class's size (`ROOT + k`) and runs between two of the
//! node's pipe cells (`PIPE`). A node hands its piece of a tunnel down to the children it
//! passes, a walk over at most four of them, crossing each face where a hash says, with its
//! blocks split by how far each child's part runs (`halve::halve`). The tunnels not yet
//! started go to the children no piece passes, by hashed weights. A pipe cell's piece hollows
//! the blocks nearest its line, as many as it holds. Only the seed makes it irregular
//! (`node_seed`).

use pxl_engine::seed::node_seed;
use pxl_spec::share::FULL;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM};
use xpute_kit::math::rng::seed_mix32;

use crate::halve::{halve, weight};
use crate::param::cave::{BLOCKS_A_TUNNEL, NUDGE, ROOT_TUNNELS};

/// The root cube, 256 units; every root holds `ROOT_TUNNELS`.
pub const ROOT: Zoom = LEAF_CELL_ZOOM - 8;
/// The pipe cell, 8 units, a tunnel's step.
pub const PIPE: Zoom = LEAF_CELL_ZOOM - 3;
/// A block, 2 units, the whole unit a pipe cell hollows.
pub const BLOCK: Zoom = LEAF_CELL_ZOOM - 1;
/// The classes a tunnel may be of, by the size of the node it starts in: a tunnel of class
/// `k` starts in a node at zoom `ROOT + k`.
const CLASS_CNT: usize = 5;
/// The blocks a root's tunnels hollow, the same in every root.
pub const ROOT_BLOCKS: u32 = {
    let mut n = 0;
    let mut k = 0;
    while k < CLASS_CNT {
        n += ROOT_TUNNELS[k] * BLOCKS_A_TUNNEL[k];
        k += 1;
    }
    n
};
/// A pipe cell's blocks, 4 a side.
pub const CELL_BLOCK_CNT: u32 = 1 << (3 * (BLOCK - PIPE));
/// A piece's side at a tunnel's end, where it crosses none.
pub const NO_SIDE: u32 = 6;

/// The tags of a node's draws (`node_seed`).
pub const SPAWN_TAG: u32 = 0xca7e_5001;
pub const WALK_TAG: u32 = 0xca7e_5002;
pub const CROSS_TAG: u32 = 0xca7e_5003;
pub const SPLIT_TAG: u32 = 0xca7e_5004;
pub const BLOCKS_TAG: u32 = 0xca7e_5006;

/// A tunnel's part in a node, in pipe cells: where it comes in and goes out, the sides it
/// crosses there (`NO_SIDE` at the tunnel's ends), and its blocks.
#[derive(Clone, Copy, Debug)]
struct Piece {
    enter: [i32; 3],
    leave: [i32; 3],
    side_in: u32,
    side_out: u32,
    blocks: u32,
}

const NONE: Piece = Piece {
    enter: [0; 3],
    leave: [0; 3],
    side_in: NO_SIDE,
    side_out: NO_SIDE,
    blocks: 0,
};

/// What a node holds: its piece of a tunnel (none: no blocks) and the
/// tunnels of each class not yet started in it.
#[derive(Clone, Copy, Debug)]
struct CaveNode {
    piece: Piece,
    tunnels: [u32; CLASS_CNT],
}

/// The tunnels of class `k` that a node at `zoom` with no piece may be handed: one at the
/// class's own zoom, and four children's worth a zoom coarser, since a node starting its own
/// takes at most four.
const fn cap(k: usize, zoom: Zoom) -> u32 {
    let class = ROOT + k as Zoom;
    if zoom > class {
        0
    } else {
        1 << (2 * (class - zoom))
    }
}

/// A node's side in pipe cells, and its first cell.
fn bounds((zoom, c): (Zoom, [i32; 3])) -> ([i32; 3], i32) {
    let s = PIPE - zoom;
    ([c[0] << s, c[1] << s, c[2] << s], 1 << s)
}

fn manhattan(a: [i32; 3], b: [i32; 3]) -> u32 {
    ((a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()) as u32
}

/// Which of a node's eight a pipe cell lies in, `dx + 2dy + 4dz`.
fn octant_of(o: [i32; 3], half: i32, p: [i32; 3]) -> u32 {
    (p[0] >= o[0] + half) as u32
        | ((p[1] >= o[1] + half) as u32) << 1
        | ((p[2] >= o[2] + half) as u32) << 2
}

/// A tunnel started in the node: from a cell picked by hash in a corner quarter of the node,
/// at any height, to one in the far quarter the other way, a little up or down. So it crosses
/// the node, and its blocks fit the pipe cells it passes, of which there are at least the
/// node's side plus one (`BLOCKS_A_TUNNEL`).
fn spawn(node: (Zoom, [i32; 3]), k: usize) -> Piece {
    let (o, s) = bounds(node);
    let h = node_seed(SPAWN_TAG, node);
    let g = seed_mix32(h, 1);
    if s < 4 {
        // Two pipe cells a side: from a cell picked by hash to the far corner, up or down as well.
        let e = [
            o[0] + (h & 1) as i32,
            o[1] + (h >> 1 & 1) as i32,
            o[2] + (h >> 2 & 1) as i32,
        ];
        let x = [
            o[0] + 1 - (h & 1) as i32,
            o[1] + 1 - (h >> 1 & 1) as i32,
            o[2] + 1 - (h >> 2 & 1) as i32,
        ];
        return Piece {
            enter: e,
            leave: x,
            side_in: NO_SIDE,
            side_out: NO_SIDE,
            blocks: BLOCKS_A_TUNNEL[k],
        };
    }
    let q = s / 4;
    let near = |r: u32, b: u32| if r >> b & 1 == 0 { (r >> (b + 1)) % q as u32 } else { (3 * q) as u32 + (r >> (b + 1)) % q as u32 } as i32;
    let ex = near(h, 0);
    let ey = near(h, 8);
    // Its height anywhere in the node, so tunnels lie at every height.
    let ez = ((h >> 16) % s as u32) as i32;
    let e = [o[0] + ex, o[1] + ey, o[2] + ez];
    // Mirrored across the node and moved back toward its middle by hash,
    // so it stays in the node.
    let back = |e: i32, r: u32| {
        let m = s - 1 - e;
        let d = (r % (q as u32 / 2 + 1)) as i32;
        if m < 2 * q {
            m + d
        } else {
            m - d
        }
    };
    let x = [
        o[0] + back(ex, g),
        o[1] + back(ey, g >> 8),
        o[2] + (ez + ((g >> 16) % (q as u32 / 2 + 1)) as i32 - q / 4).clamp(0, s - 1),
    ];
    Piece {
        enter: e,
        leave: x,
        side_in: NO_SIDE,
        side_out: NO_SIDE,
        blocks: BLOCKS_A_TUNNEL[k],
    }
}

/// Whether a walk whose ends differ on the sides `d` steps aside and back, by hash.
fn is_aside(d: u32, h: u32) -> bool {
    d != 0 && d.count_ones() == 1 && h & 1 == 1
}

/// The walk from child `u` to child `v` over at most four children: its length, and its
/// `i`-th child (`walk_at`). It crosses the sides in which they differ in an order picked by
/// hash, or, where only one side differs, takes a step aside and back, picked by hash.
fn walk_len(u: u32, v: u32, h: u32) -> u32 {
    let d = u ^ v;
    if is_aside(d, h) {
        4
    } else {
        d.count_ones() + 1
    }
}

fn walk_at(u: u32, v: u32, h: u32, i: u32) -> u32 {
    let d = u ^ v;
    if is_aside(d, h) {
        // Aside by the next side or the one after, by hash, then across, then back.
        let b = if (h >> 1) & 1 == 0 {
            ((d << 1) | (d >> 2)) & 7
        } else {
            ((d << 2) | (d >> 1)) & 7
        };
        return match i {
            0 => u,
            1 => u ^ b,
            2 => u ^ b ^ d,
            _ => v,
        };
    }
    // The first `i` differing sides from a rotation by hash.
    let r = (h >> 2) % 3;
    let mut cur = u;
    let mut flipped = 0;
    let mut j = 0;
    while j < 3 && flipped < i {
        let bit = 1 << ((j + r) % 3);
        if d & bit != 0 {
            cur ^= bit;
            flipped += 1;
        }
        j += 1;
    }
    cur
}

/// The node's piece handed down to its children: child `want`'s part of it, and the children
/// the walk passes, one bit each. The walk's parts cross each face near the way on toward its
/// end, moved by hash, and its blocks are split by how far each part runs, none past a pipe
/// cell's room each (`halve`).
fn hand_down(node: (Zoom, [i32; 3]), p: Piece, want: u32) -> (Piece, u32) {
    let (o, s) = bounds(node);
    let half = s / 2;
    let (u, v, wh) = (
        octant_of(o, half, p.enter),
        octant_of(o, half, p.leave),
        node_seed(WALK_TAG, node),
    );
    let n = walk_len(u, v, wh);
    let mut on = 0u32;
    let mut mine = NONE;
    let mut at = 8u32;
    let mut cells = [0u32; 4];
    let mut cur = p.enter;
    let mut from = p.side_in;
    let mut i = 0;
    while i < n {
        let k = walk_at(u, v, wh, i);
        on |= 1 << k;
        let co = [
            o[0] + half * (k & 1) as i32,
            o[1] + half * (k >> 1 & 1) as i32,
            o[2] + half * (k >> 2 & 1) as i32,
        ];
        let mut part = Piece {
            enter: cur,
            leave: p.leave,
            side_in: from,
            side_out: p.side_out,
            blocks: 0,
        };
        if i + 1 < n {
            let nk = walk_at(u, v, wh, i + 1);
            let axis = (k ^ nk).trailing_zeros();
            let up = nk >> axis & 1 == 1;
            let h = seed_mix32(node_seed(CROSS_TAG, node), i);
            let mut j = 0;
            while j < 3 {
                let v = (cur[j] + p.leave[j]) / 2
                    + ((h >> (10 * j)) & 0x3ff) as i32 * (half / 2 + 1) / 1024
                    - half / 4;
                part.leave[j] = if v < co[j] {
                    co[j]
                } else if v > co[j] + half - 1 {
                    co[j] + half - 1
                } else {
                    v
                };
                j += 1;
            }
            part.leave[axis as usize] = if up {
                co[axis as usize] + half - 1
            } else {
                co[axis as usize]
            };
            part.side_out = 2 * axis + up as u32;
            cur = part.leave;
            cur[axis as usize] += if up { 1 } else { -1 };
            from = 2 * axis + !up as u32;
        }
        cells[i as usize] = manhattan(part.enter, part.leave) + 1;
        if k == want {
            mine = part;
            at = i;
        }
        i += 1;
    }
    // Its blocks along the walk, a part at a time: each part takes its share of what is left, by
    // its cells over the cells left, within its pipe cells' room and the rest's (`halve`).
    let mut rest = 0u32;
    let mut i = 0;
    while i < n {
        rest += cells[i as usize];
        i += 1;
    }
    let mut left = p.blocks;
    let mut i = 0;
    while i < n && i <= at {
        let c = cells[i as usize];
        rest -= c;
        let all = c + rest;
        let share = (c << 16) / if all == 0 { 1 } else { all };
        let share = if share > 0xffff { 0xffff } else { share };
        let w = weight(share, wh.rotate_left(7 * i + 3));
        let got = halve(left, [c * CELL_BLOCK_CNT, rest * CELL_BLOCK_CNT], w);
        if i == at {
            mine.blocks = got;
        }
        left -= got;
        i += 1;
    }
    (mine, on)
}

/// The node's tunnels not yet started, handed to the children no piece passes: child
/// `want`'s share. Each class is halved across, then along, then up (`halve`), by a hash's
/// weight alike on each side, each half's room being what its children can start (`cap`).
fn tunnels_down(
    node: (Zoom, [i32; 3]),
    tunnels: [u32; CLASS_CNT],
    on: u32,
    want: u32,
) -> [u32; CLASS_CNT] {
    let child = node.0 + 1;
    let mut out = [0u32; CLASS_CNT];
    let mut k = 0;
    while k < CLASS_CNT {
        let r = cap(k, child);
        let mut n = tunnels[k];
        let h = if n > 0 {
            seed_mix32(node_seed(SPLIT_TAG, node), k as u32)
        } else {
            0
        };
        // The children still in play and free of a piece, one bit each, narrowed a side at a time;
        // a half's room is that of its children in play.
        let mut keep = 0xffu32 & !on;
        let mut side = 0;
        while side < 3 && n > 0 {
            let upper = match side {
                0 => 0xaau32,
                1 => 0xcc,
                _ => 0xf0,
            };
            let caps = [
                (keep & !upper).count_ones() * r,
                (keep & upper).count_ones() * r,
            ];
            let u = (h >> (8 * side)) & 0xff;
            let w = weight(0x4000 + u * 0x80, h.rotate_left(11 * side + 5));
            let lo = halve(n, caps, w);
            let up = want >> side & 1 == 1;
            n = if up { n - lo } else { lo };
            keep &= if up { upper } else { !upper };
            side += 1;
        }
        out[k] = n;
        k += 1;
    }
    out
}

/// What the node holds, from its root down.
fn holds(node: (Zoom, [i32; 3])) -> CaveNode {
    let (zoom, c) = node;
    let at = |z: Zoom| [c[0] >> (zoom - z), c[1] >> (zoom - z), c[2] >> (zoom - z)];
    let mut h = CaveNode {
        piece: NONE,
        tunnels: ROOT_TUNNELS,
    };
    let mut z = ROOT;
    loop {
        let here = (z, at(z));
        // Start its own where it is its class's size and holds one.
        let k = (z - ROOT) as usize;
        if k < CLASS_CNT && h.tunnels[k] > 0 && h.piece.blocks == 0 {
            h.piece = spawn(here, k);
            h.tunnels[k] -= 1;
        }
        if z == zoom || z == PIPE || h.piece.blocks == 0 && h.tunnels == [0; CLASS_CNT] {
            return h;
        }
        let next = at(z + 1);
        let i = (next[0] & 1 | (next[1] & 1) << 1 | (next[2] & 1) << 2) as u32;
        let (piece, on) = if h.piece.blocks == 0 {
            (NONE, 0)
        } else {
            hand_down(here, h.piece, i)
        };
        h = CaveNode {
            piece,
            tunnels: tunnels_down(here, h.tunnels, on, i),
        };
        z += 1;
    }
}

/// How far block `b`'s middle lies from the piece's line, which runs from the middle of the
/// side it comes in by to the middle of the side it goes out by, in eighths of a unit. It is
/// given as the squared distance times the line's squared length (`l`), so no division is
/// needed, and nudged by a draw from the cell's hash `h` so that no wall runs smooth. The
/// lower the key, the sooner the block is hollowed.
fn key(a: [i32; 3], ab: [i32; 3], l: i32, h: u32, b: u32) -> i32 {
    let ap = [
        (b & 3) as i32 * 16 + 8 - a[0],
        (b >> 2 & 3) as i32 * 16 + 8 - a[1],
        (b >> 4 & 3) as i32 * 16 + 8 - a[2],
    ];
    let pp = ap[0] * ap[0] + ap[1] * ap[1] + ap[2] * ap[2];
    let t = ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2];
    // |ap|²·l less t² along the line; past its ends, the distance to the end.
    let d = if l == 0 || t <= 0 {
        pp * if l == 0 { 1 } else { l }
    } else if t >= l {
        (pp - 2 * t + l) * l
    } else {
        pp * l - t * t
    };
    d + (seed_mix32(h, b) % NUDGE) as i32 * if l == 0 { 1 } else { l }
}

/// Side `side`'s middle in a pipe cell, in eighths of a unit from its
/// corner; the cell's middle for `NO_SIDE`.
fn side_middle(side: u32) -> [i32; 3] {
    let mut p = [32; 3];
    if side < NO_SIDE {
        p[(side / 2) as usize] = if side & 1 == 1 { 64 } else { 0 };
    }
    p
}

/// How many of a pipe cell's blocks, in the cube `side` a side from block `lo` (`side` 1 or
/// 2, `t` 0 or 1), are hollow, among the piece's `blocks` nearest its line (`key`, the lower block first where
/// two tie), where the cell's hash is `h`. Each block's key
/// is read once, every block of the cell is ranked against them all at once, and the count
/// stops once each is ranked past the piece's blocks.
fn hollow_from(h: u32, p: &Piece, lo: [u32; 3], t: u32) -> u32 {
    let n = 1u32 << (3 * t);
    if p.blocks >= CELL_BLOCK_CNT {
        return n;
    }
    let a = side_middle(p.side_in);
    let z = side_middle(p.side_out);
    let ab = [z[0] - a[0], z[1] - a[1], z[2] - a[2]];
    let l = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let side = 1u32 << t;
    let mut ids = [0u32; 8];
    let mut mine = [0i32; 8];
    let mut i = 0;
    while i < n {
        let at = [i & (side - 1), (i >> t) & (side - 1), i >> (2 * t)];
        ids[i as usize] = (lo[0] + at[0]) | (lo[1] + at[1]) << 2 | (lo[2] + at[2]) << 4;
        mine[i as usize] = key(a, ab, l, h, ids[i as usize]);
        i += 1;
    }
    let mut before = [0u32; 8];
    let mut past = 0;
    let mut o = 0;
    while o < CELL_BLOCK_CNT && past < n {
        let k = key(a, ab, l, h, o);
        let mut i = 0;
        while i < n {
            let m = mine[i as usize];
            if before[i as usize] < p.blocks && (k < m || k == m && o < ids[i as usize]) {
                before[i as usize] += 1;
                past += (before[i as usize] == p.blocks) as u32;
            }
            i += 1;
        }
        o += 1;
    }
    n - past
}

/// The blocks of the caves in the node.
fn blocks(node: (Zoom, [i32; 3])) -> u32 {
    let (zoom, c) = node;
    if zoom <= PIPE {
        let h = holds(node);
        let mut n = h.piece.blocks;
        let mut k = 0;
        while k < CLASS_CNT {
            n += h.tunnels[k] * BLOCKS_A_TUNNEL[k];
            k += 1;
        }
        return n;
    }
    // Finer than a pipe cell: its blocks among its cell's.
    let s = zoom - PIPE;
    let cell = [c[0] >> s, c[1] >> s, c[2] >> s];
    let p = holds((PIPE, cell)).piece;
    if p.blocks == 0 {
        return 0;
    }
    // The node's blocks, `1 << t` a side: one at the block zoom or finer, and 2 a side
    // otherwise.
    let (t, lo) = if zoom >= BLOCK {
        let t = zoom - BLOCK;
        (0, [(c[0] >> t) & 3, (c[1] >> t) & 3, (c[2] >> t) & 3])
    } else {
        let t = BLOCK - zoom;
        (t, [(c[0] << t) & 3, (c[1] << t) & 3, (c[2] << t) & 3])
    };
    let h = node_seed(BLOCKS_TAG, (PIPE, cell));
    hollow_from(h, &p, lo.map(|v| v as u32), t)
}

/// The share of the cube `c` at `zoom` that the caves take, in `share::FULL`'s units: its
/// blocks of caves over its volume, or a root's at any coarser zoom. It is kept as one body
/// (`inline(never)`) however many places ask it: the ground asks it of a cell and of a node.
#[inline(never)]
pub fn hollow(node: (Zoom, [i32; 3])) -> u32 {
    let (zoom, c) = node;
    // Finer than a leaf, its leaf's.
    let (zoom, c) = if zoom > LEAF_CELL_ZOOM {
        let t = zoom - LEAF_CELL_ZOOM;
        (LEAF_CELL_ZOOM, [c[0] >> t, c[1] >> t, c[2] >> t])
    } else {
        (zoom, c)
    };
    // Coarser than a root, a root's: every root holds the same.
    let (zoom, n) = if zoom <= ROOT {
        (ROOT, ROOT_BLOCKS)
    } else {
        (zoom, blocks((zoom, c)))
    };
    // A block's leaves, over the node's, in `FULL`'s units.
    let shift = (3 * (LEAF_CELL_ZOOM - BLOCK) + FULL.trailing_zeros()) as i32
        - 3 * (LEAF_CELL_ZOOM - zoom) as i32;
    if shift >= 0 {
        let v = n << shift;
        if v > FULL {
            FULL
        } else {
            v
        }
    } else {
        n >> -shift
    }
}

#[cfg(test)]
#[path = "cave.test.rs"]
mod test;
