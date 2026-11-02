// pxl-content/cave.rs

//! gpu-portable: the caves, worms of tunnel decided from the top down, so a
//! node's share of cave is its own before anything under it is asked and a
//! coarse cell holds exactly what its leaves do.
//!
//! A root cube (`ROOT`) holds the same tunnels as every other, so every node
//! coarser is the same share. A tunnel starts in a node of its class's size
//! (`CLASS`) and runs between two of its pipe cells (`PIPE`); a node hands
//! its piece of a tunnel to the children it passes, a walk over at most four
//! of them, crossing each face where a hash says, its blocks split by how
//! far each child's part runs (`share::halve`); the tunnels not yet
//! started go to the children no piece passes, by hashed weights. A pipe cell's piece hollows the blocks nearest its line, as many
//! as it holds. Irregular by the seed alone (`node_seed`).

use pxl_engine::answer::FULL;
use pxl_engine::seed::node_seed;
use pxl_engine::{Zoom, LEAF_CELL};

use crate::param::cave::{BLOCKS_A_TUNNEL, NUDGE, ROOT_TUNNELS};
use crate::share::halve;

/// The root cube, 256 units: every one holds `ROOT_TUNNELS`.
pub const ROOT: Zoom = LEAF_CELL - 8;
/// The pipe cell, 8 units, a tunnel's step.
pub const PIPE: Zoom = LEAF_CELL - 3;
/// A block, 2 units, what a pipe cell hollows a whole of.
const BLOCK: Zoom = LEAF_CELL - 1;
/// The classes' zooms: a tunnel of class `k` starts in a node there.
/// The classes a tunnel is of, by the size of the node it starts in.
const CLASSES: usize = 5;
const CLASS: [Zoom; CLASSES] = [ROOT, ROOT + 1, ROOT + 2, ROOT + 3, ROOT + 4];
/// A root's blocks: its tunnels', every root alike.
const ROOT_BLOCKS: u32 = {
    let mut n = 0;
    let mut k = 0;
    while k < CLASSES {
        n += ROOT_TUNNELS[k] * BLOCKS_A_TUNNEL[k];
        k += 1;
    }
    n
};
/// A pipe cell's blocks, 4 a side.
const CELL_BLOCKS: u32 = 64;

/// The tags of a node's draws (`node_seed`).
const SPAWN: u32 = 0xca7e_5001;
const WALK: u32 = 0xca7e_5002;
const CROSS: u32 = 0xca7e_5003;
const SPLIT: u32 = 0xca7e_5004;
const BLOCKS: u32 = 0xca7e_5006;

/// A tunnel's part in a node, in pipe cells: where it comes in and goes
/// out, the sides it crosses there (6 none, at its ends), and its blocks.
#[derive(Clone, Copy, Debug)]
struct Piece {
    e: [i32; 3],
    x: [i32; 3],
    from: u32,
    to: u32,
    blocks: u32,
}

const NONE: Piece = Piece {
    e: [0; 3],
    x: [0; 3],
    from: 6,
    to: 6,
    blocks: 0,
};

/// What a node holds: its piece of a tunnel (none: no blocks) and the
/// tunnels of each class not yet started in it.
#[derive(Clone, Copy, Debug)]
struct Holds {
    piece: Piece,
    tunnels: [u32; CLASSES],
}

/// Tunnels of class `k` a node at `zoom` with no piece may be handed: one at
/// the class's own zoom, four children's a step coarser (a node starting its
/// own takes at most four).
const fn room(k: usize, zoom: Zoom) -> u32 {
    if zoom > CLASS[k] {
        0
    } else {
        1 << (2 * (CLASS[k] - zoom))
    }
}

/// The blocks a tunnel of class `k` hollows.
const fn tunnel_blocks(k: usize) -> u32 {
    BLOCKS_A_TUNNEL[k]
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
fn child_of(o: [i32; 3], half: i32, p: [i32; 3]) -> u32 {
    (p[0] >= o[0] + half) as u32
        | ((p[1] >= o[1] + half) as u32) << 1
        | ((p[2] >= o[2] + half) as u32) << 2
}

/// A tunnel started in the node: from a cell by hash in a corner quarter of
/// the node across, at any height, to one in the far quarter the other way, a
/// little up or down: so it crosses the node and its blocks fit the pipe cells it passes
/// (at least its side and one, `BLOCKS_A_TUNNEL`).
fn spawn(node: (Zoom, [i32; 3]), k: usize) -> Piece {
    let (o, s) = bounds(node);
    let h = node_seed(SPAWN, node);
    let g = node_seed(SPAWN ^ 1, node);
    if s < 4 {
        // Two pipe cells a side: from a cell by hash to the far corner, up or
        // down too.
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
            e,
            x,
            from: 6,
            to: 6,
            blocks: tunnel_blocks(k),
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
        e,
        x,
        from: 6,
        to: 6,
        blocks: tunnel_blocks(k),
    }
}

/// The walk from child `u` to child `v` over at most four: its length, and
/// its `i`-th child (`walk_at`). Their differing sides in an order by hash,
/// or, where one side differs, a step aside and back by hash.
fn walk_len(u: u32, v: u32, h: u32) -> u32 {
    let d = u ^ v;
    if d != 0 && d.count_ones() == 1 && h & 1 == 1 {
        4
    } else {
        d.count_ones() + 1
    }
}

fn walk_at(u: u32, v: u32, h: u32, i: u32) -> u32 {
    let d = u ^ v;
    if d != 0 && d.count_ones() == 1 && h & 1 == 1 {
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

/// The node's piece handed to its children, child `want`'s part of it, and
/// the children the walk passes, a bit each: the walk's parts cross each face
/// near the way on toward its end, moved by hash; their blocks split by how
/// far each runs, none past a pipe cell's each (`halve`).
fn hand_down(node: (Zoom, [i32; 3]), p: Piece, want: u32) -> (Piece, u32) {
    let (o, s) = bounds(node);
    let half = s / 2;
    let (u, v, wh) = (
        child_of(o, half, p.e),
        child_of(o, half, p.x),
        node_seed(WALK, node),
    );
    let n = walk_len(u, v, wh);
    let mut on = 0u32;
    let mut mine = NONE;
    let mut at = 8u32;
    let mut cells = [0u32; 4];
    let mut cur = p.e;
    let mut from = p.from;
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
            e: cur,
            x: p.x,
            from,
            to: p.to,
            blocks: 0,
        };
        if i + 1 < n {
            let nk = walk_at(u, v, wh, i + 1);
            let axis = (k ^ nk).trailing_zeros();
            let up = nk >> axis & 1 == 1;
            let h = node_seed(CROSS ^ i, node);
            let mut j = 0;
            while j < 3 {
                let v = (cur[j] + p.x[j]) / 2
                    + ((h >> (10 * j)) & 0x3ff) as i32 * (half / 2 + 1) / 1024
                    - half / 4;
                part.x[j] = if v < co[j] {
                    co[j]
                } else if v > co[j] + half - 1 {
                    co[j] + half - 1
                } else {
                    v
                };
                j += 1;
            }
            part.x[axis as usize] = if up {
                co[axis as usize] + half - 1
            } else {
                co[axis as usize]
            };
            part.to = 2 * axis + up as u32;
            cur = part.x;
            cur[axis as usize] += if up { 1 } else { -1 };
            from = 2 * axis + !up as u32;
        }
        cells[i as usize] = manhattan(part.e, part.x) + 1;
        if k == want {
            mine = part;
            at = i;
        }
        i += 1;
    }
    // Its blocks along the walk, a part at a time: each its share of what
    // is left by its cells of the cells left, within its pipe cells' room
    // and the rest's (`halve`).
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
        let weight = (c << 16) / if all == 0 { 1 } else { all };
        let weight = if weight > 0xffff { 0xffff } else { weight };
        let w = weight << 16 | (wh.rotate_left(7 * i + 3) & 0xffff);
        let got = halve(left, [c * CELL_BLOCKS, rest * CELL_BLOCKS], w);
        if i == at {
            mine.blocks = got;
        }
        left -= got;
        i += 1;
    }
    (mine, on)
}

/// The node's tunnels not yet started handed to its children no piece
/// passes, child `want`'s: each class halved across, then along, then up
/// (`halve`), by a hash's weight alike each side, each half's room what its
/// children can start (`room`).
fn tunnels_down(
    node: (Zoom, [i32; 3]),
    tunnels: [u32; CLASSES],
    on: u32,
    want: u32,
) -> [u32; CLASSES] {
    let child = node.0 + 1;
    let mut out = [0u32; CLASSES];
    let mut k = 0;
    while k < CLASSES {
        let r = room(k, child);
        let mut n = tunnels[k];
        let h = if n > 0 {
            node_seed(SPLIT ^ k as u32, node)
        } else {
            0
        };
        // The children still in play and free of a piece, a bit each,
        // narrowed a side at a time; a half's room its children's in play.
        let mut keep = 0xffu32 & !on;
        let mut side = 0;
        while side < 3 && n > 0 {
            let upper = match side {
                0 => 0xaau32,
                1 => 0xcc,
                _ => 0xf0,
            };
            let rooms = [
                (keep & !upper).count_ones() * r,
                (keep & upper).count_ones() * r,
            ];
            let u = (h >> (8 * side)) & 0xff;
            let weight = 0x4000 + u * 0x80;
            let w = weight << 16 | (h.rotate_left(11 * side + 5) & 0xffff);
            let lo = halve(n, rooms, w);
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
fn holds(node: (Zoom, [i32; 3])) -> Holds {
    let (zoom, c) = node;
    let at = |z: Zoom| [c[0] >> (zoom - z), c[1] >> (zoom - z), c[2] >> (zoom - z)];
    let mut h = Holds {
        piece: NONE,
        tunnels: ROOT_TUNNELS,
    };
    let mut z = ROOT;
    loop {
        let here = (z, at(z));
        // Start its own where it is its class's size and holds one.
        let k = (z - ROOT) as usize;
        if k < CLASSES && h.tunnels[k] > 0 && h.piece.blocks == 0 {
            h.piece = spawn(here, k);
            h.tunnels[k] -= 1;
        }
        if z == zoom
            || z == PIPE
            || h.piece.blocks == 0
                && h.tunnels[0] + h.tunnels[1] + h.tunnels[2] + h.tunnels[3] + h.tunnels[4] == 0
        {
            return h;
        }
        let next = at(z + 1);
        let i = (next[0] & 1 | (next[1] & 1) << 1 | (next[2] & 1) << 2) as u32;
        let (piece, on) = if h.piece.blocks == 0 {
            (NONE, 0)
        } else {
            hand_down(here, h.piece, i)
        };
        h = Holds {
            piece,
            tunnels: tunnels_down(here, h.tunnels, on, i),
        };
        z += 1;
    }
}

/// How far block `b`'s middle lies from the piece's line, from the middle
/// of the side it comes in by to the middle of the side it goes out by, in
/// eighths of a unit, its square times the line's (`l`, so no division),
/// nudged by a draw off the cell's hash `h` so no wall runs smooth: the
/// lower, the sooner the block is hollowed.
fn key(a: [i32; 3], ab: [i32; 3], l: i32, h: u32, b: u32) -> i32 {
    let ap = [
        (b & 3) as i32 * 16 + 8 - a[0],
        (b >> 2 & 3) as i32 * 16 + 8 - a[1],
        (b >> 4 & 3) as i32 * 16 + 8 - a[2],
    ];
    let pp = ap[0] * ap[0] + ap[1] * ap[1] + ap[2] * ap[2];
    let t = ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2];
    // |ap|²·l less t² along it, its ends' own distances past them.
    let d = if l == 0 || t <= 0 {
        pp * if l == 0 { 1 } else { l }
    } else if t >= l {
        (pp - 2 * t + l) * l
    } else {
        pp * l - t * t
    };
    let mut n = h ^ b.wrapping_mul(0x9e37_79b9);
    n ^= n >> 15;
    n = n.wrapping_mul(0x2c1b_3c6d);
    n ^= n >> 12;
    d + (n % NUDGE) as i32 * if l == 0 { 1 } else { l }
}

/// Side `side`'s middle in a pipe cell, in eighths of a unit from its
/// corner; the cell's middle for none (6).
fn side_mid(side: u32) -> [i32; 3] {
    let mut p = [32; 3];
    if side < 6 {
        p[(side / 2) as usize] = if side & 1 == 1 { 64 } else { 0 };
    }
    p
}

/// Whether block `b` of a pipe cell whose hash is `h` is hollow: among its
/// piece's `blocks` nearest its line (`key`, the lower block first where two
/// tie).
fn block_hollow(h: u32, p: &Piece, b: u32) -> bool {
    if p.blocks >= CELL_BLOCKS {
        return true;
    }
    let a = side_mid(p.from);
    let z = side_mid(p.to);
    let ab = [z[0] - a[0], z[1] - a[1], z[2] - a[2]];
    let l = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let mine = key(a, ab, l, h, b);
    let mut before = 0;
    let mut o = 0;
    while o < CELL_BLOCKS && before < p.blocks {
        let k = key(a, ab, l, h, o);
        if k < mine || k == mine && o < b {
            before += 1;
        }
        o += 1;
    }
    before < p.blocks
}

/// The blocks of the caves in the node.
fn blocks(node: (Zoom, [i32; 3])) -> u32 {
    let (zoom, c) = node;
    if zoom <= PIPE {
        let h = holds(node);
        let mut n = h.piece.blocks;
        let mut k = 0;
        while k < CLASSES {
            n += h.tunnels[k] * tunnel_blocks(k);
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
    // The node's blocks: below the block zoom one, else 2 or 4 a side.
    let (side, lo) = if zoom >= BLOCK {
        let t = zoom - BLOCK;
        (1, [(c[0] >> t) & 3, (c[1] >> t) & 3, (c[2] >> t) & 3])
    } else {
        let t = BLOCK - zoom;
        let n = 1 << t;
        (n, [(c[0] << t) & 3, (c[1] << t) & 3, (c[2] << t) & 3])
    };
    let h = node_seed(BLOCKS, (PIPE, cell));
    let mut n = 0;
    let mut z = 0;
    while z < side {
        let mut y = 0;
        while y < side {
            let mut x = 0;
            while x < side {
                let b = (lo[0] + x) as u32 | ((lo[1] + y) as u32) << 2 | ((lo[2] + z) as u32) << 4;
                n += block_hollow(h, &p, b) as u32;
                x += 1;
            }
            y += 1;
        }
        z += 1;
    }
    n
}

/// The share of the cube `c` at `zoom` the caves take, in `answer::FULL`'s
/// units: its blocks of caves over its volume, a root's at any coarser.
/// One body however many ask it (the ground asks it of a cell and of a node).
#[inline(never)]
pub fn hollow(node: (Zoom, [i32; 3])) -> u32 {
    let (zoom, c) = node;
    // Finer than a leaf, its leaf's.
    let (zoom, c) = if zoom > LEAF_CELL {
        let t = zoom - LEAF_CELL;
        (LEAF_CELL, [c[0] >> t, c[1] >> t, c[2] >> t])
    } else {
        (zoom, c)
    };
    // Coarser than a root, a root's: every root holds the same.
    let (zoom, n) = if zoom <= ROOT {
        (ROOT, ROOT_BLOCKS)
    } else {
        (zoom, blocks((zoom, c)))
    };
    // A block is 8 leaves; FULL is 2^15; the node holds 2^(3(LEAF − zoom)) leaves.
    let shift = 3 + 15 - 3 * (LEAF_CELL - zoom) as i32;
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
