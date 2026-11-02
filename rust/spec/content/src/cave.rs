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
//! far each child's part runs (`pyramid::share_out`); the tunnels not yet
//! started go to the children no piece passes, by hashed weights heavier
//! below. A pipe cell's piece hollows the blocks nearest its line, as many
//! as it holds. Irregular by the seed alone (`node_seed`).

use pxl_engine::answer::FULL;
use pxl_engine::seed::node_seed;
use pxl_engine::{Zoom, LEAF_CELL};

use crate::param::cave::{BLOCKS_A_TUNNEL, NUDGE, ROOT_TUNNELS};
use crate::pyramid::share_out;

/// The root cube, 256 units: every one holds `ROOT_TUNNELS`.
pub const ROOT: Zoom = LEAF_CELL - 8;
/// The pipe cell, 8 units, a tunnel's step.
pub const PIPE: Zoom = LEAF_CELL - 3;
/// A block, 2 units, what a pipe cell hollows a whole of.
const BLOCK: Zoom = LEAF_CELL - 1;
/// The classes' zooms: a tunnel of class `k` starts in a node there.
const CLASS: [Zoom; 4] = [ROOT, ROOT + 1, ROOT + 2, ROOT + 3];
/// A pipe cell's blocks, 4 a side.
const CELL_BLOCKS: u32 = 64;

/// The tags of a node's draws (`node_seed`).
const SPAWN: u32 = 0xca7e_5001;
const WALK: u32 = 0xca7e_5002;
const CROSS: u32 = 0xca7e_5003;
const SPLIT: u32 = 0xca7e_5004;
const FACE: u32 = 0xca7e_5005;
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
    tunnels: [u32; 4],
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
    (p[0] >= o[0] + half) as u32 | ((p[1] >= o[1] + half) as u32) << 1 | ((p[2] >= o[2] + half) as u32) << 2
}

/// A tunnel started in the node: from a cell by hash in the node's middle
/// half to one half the node's side away across, toward the side with room,
/// and a little up or down; so its blocks fit the pipe cells it must pass.
fn spawn(node: (Zoom, [i32; 3]), k: usize) -> Piece {
    let (o, s) = bounds(node);
    let h = node_seed(SPAWN, node);
    let g = node_seed(SPAWN ^ 1, node);
    let q = s / 4;
    let e = [o[0] + q + (h % (2 * q) as u32) as i32, o[1] + q + ((h >> 8) % (2 * q) as u32) as i32, o[2] + q + ((h >> 16) % (2 * q) as u32) as i32];
    // |dx| + |dy| = s/2, the share across by hash, each side's sign by hash.
    let across = (g % (2 * q + 1) as u32) as i32;
    // Toward the side with room, so it stays in the node.
    let dx = if e[0] - o[0] < 2 * q { across } else { -across };
    let dy = if e[1] - o[1] < 2 * q { 2 * q - across } else { across - 2 * q };
    let dz = ((g >> 10) % (q as u32 / 2 + 1)) as i32 - q / 4;
    let x = [e[0] + dx, e[1] + dy, e[2] + dz];
    Piece { e, x, from: 6, to: 6, blocks: tunnel_blocks(k) }
}

/// The walk from child `u` to child `v` over at most four: their differing
/// sides in an order by hash, or, where one side differs, a step aside and
/// back by hash.
fn walk(u: u32, v: u32, h: u32) -> ([u32; 4], u32) {
    let d = u ^ v;
    let mut w = [u; 4];
    if d == 0 {
        return (w, 1);
    }
    if d.count_ones() == 1 && h & 1 == 1 {
        let b = if (h >> 1) & 1 == 0 { ((d << 1) | (d >> 2)) & 7 } else { ((d << 2) | (d >> 1)) & 7 };
        w[1] = u ^ b;
        w[2] = u ^ b ^ d;
        w[3] = v;
        return (w, 4);
    }
    // The differing sides, a rotation by hash.
    let mut bits = [0u32; 3];
    let mut n = 0;
    let r = (h >> 2) % 3;
    let mut i = 0;
    while i < 3 {
        let b = 1 << ((i + r) % 3);
        if d & b != 0 {
            bits[n] = b;
            n += 1;
        }
        i += 1;
    }
    let mut cur = u;
    let mut i = 0;
    while i < n {
        cur ^= bits[i];
        w[i + 1] = cur;
        i += 1;
    }
    (w, n as u32 + 1)
}

/// The node's piece handed to its children: each child the walk passes its
/// part, crossing each face at a cell by hash, its blocks by how far its part
/// runs; and child `want`'s part.
fn hand_down(node: (Zoom, [i32; 3]), p: Piece) -> ([Piece; 8], u32) {
    // The children it passes, a bit each.
    let mut on = 0u32;
    let (o, s) = bounds(node);
    let half = s / 2;
    let (w, n) = walk(child_of(o, half, p.e), child_of(o, half, p.x), node_seed(WALK, node));
    let mut parts = [NONE; 8];
    let mut cur = p.e;
    let mut from = p.from;
    let mut i = 0;
    while i < n {
        let k = w[i as usize];
        on |= 1 << k;
        let co = [o[0] + half * (k & 1) as i32, o[1] + half * (k >> 1 & 1) as i32, o[2] + half * (k >> 2 & 1) as i32];
        if i + 1 == n {
            parts[k as usize] = Piece { e: cur, x: p.x, from, to: p.to, blocks: 0 };
        } else {
            let nk = w[i as usize + 1];
            let axis = (k ^ nk).trailing_zeros();
            let up = nk >> axis & 1 == 1;
            // Where the way on toward its end meets the face, moved by up to a
            // quarter of the child by hash.
            let h = node_seed(CROSS ^ i, node);
            let mut x = [0; 3];
            let mut j = 0;
            while j < 3 {
                let mid = (cur[j] + p.x[j]) / 2;
                let jit = ((h >> (10 * j)) & 0x3ff) as i32 * (half / 2 + 1) / 1024 - half / 4;
                let v = mid + jit;
                x[j] = if v < co[j] { co[j] } else if v > co[j] + half - 1 { co[j] + half - 1 } else { v };
                j += 1;
            }
            x[axis as usize] = if up { co[axis as usize] + half - 1 } else { co[axis as usize] };
            let side = 2 * axis + up as u32;
            parts[k as usize] = Piece { e: cur, x, from, to: side, blocks: 0 };
            cur = x;
            cur[axis as usize] += if up { 1 } else { -1 };
            from = 2 * axis + !up as u32;
        }
        i += 1;
    }
    // Its blocks by how far each part runs, none past a pipe cell's each.
    let mut rooms = [0u32; 8];
    let mut weights = [0u32; 8];
    let mut i = 0;
    while i < n {
        let k = w[i as usize] as usize;
        let cells = manhattan(parts[k].e, parts[k].x) + 1;
        rooms[k] = cells * CELL_BLOCKS;
        weights[k] = cells * 64;
        i += 1;
    }
    let got = share_out(p.blocks, rooms, weights);
    let mut k = 0;
    while k < 8 {
        parts[k].blocks = got[k];
        k += 1;
    }
    (parts, on)
}

/// The node's tunnels not yet started to its children no piece passes: each
/// class by hashed weights, the lower four heavier, a child's room four of
/// its children's where it starts its own and none where a piece passes.
fn tunnels_down(node: (Zoom, [i32; 3]), tunnels: [u32; 4], on: u32) -> [[u32; 4]; 8] {
    let child = node.0 + 1;
    let mut out = [[0u32; 4]; 8];
    let mut k = 0;
    while k < 4 {
        if tunnels[k] > 0 {
            let mut rooms = [0u32; 8];
            let mut weights = [0u32; 8];
            let h = node_seed(SPLIT ^ k as u32, node);
            let mut i = 0;
            while i < 8 {
                if on >> i & 1 == 0 {
                    rooms[i] = room(k, child);
                }
                let u = (h.rotate_left(4 * i as u32) & 0x3fff) + 0x800;
                weights[i] = if i & 4 == 0 { u * 3 / 2 } else { u };
                i += 1;
            }
            let got = share_out(tunnels[k], rooms, weights);
            let mut i = 0;
            while i < 8 {
                out[i][k] = got[i];
                i += 1;
            }
        }
        k += 1;
    }
    out
}

/// What the node holds, from its root down.
fn holds(node: (Zoom, [i32; 3])) -> Holds {
    let (zoom, c) = node;
    let at = |z: Zoom| [c[0] >> (zoom - z), c[1] >> (zoom - z), c[2] >> (zoom - z)];
    let mut h = Holds { piece: NONE, tunnels: ROOT_TUNNELS };
    let mut z = ROOT;
    loop {
        let here = (z, at(z));
        // Start its own where it is its class's size and holds one.
        let mut k = 0;
        while k < 4 {
            if CLASS[k] == z && h.tunnels[k] > 0 && h.piece.blocks == 0 {
                h.piece = spawn(here, k);
                h.tunnels[k] -= 1;
            }
            k += 1;
        }
        if z == zoom || z == PIPE {
            return h;
        }
        let (parts, on) = if h.piece.blocks == 0 {
            ([NONE; 8], 0)
        } else {
            hand_down(here, h.piece)
        };
        let down = tunnels_down(here, h.tunnels, on);
        let next = at(z + 1);
        let i = (next[0] & 1 | (next[1] & 1) << 1 | (next[2] & 1) << 2) as usize;
        h = Holds { piece: parts[i], tunnels: down[i] };
        z += 1;
    }
}

/// A face's crossing point in its pipe cell, in sixteenths of a unit from
/// the cell's corner: the middle of side `side`, moved across it by the
/// face's own hash, so the cells either side meet at one point.
fn crossing(cell: [i32; 3], side: u32) -> [i32; 3] {
    let mid = 4 * 16;
    if side >= 6 {
        return [mid; 3];
    }
    let axis = (side / 2) as usize;
    let up = side & 1 == 1;
    let mut lower = cell;
    if !up {
        lower[axis] -= 1;
    }
    let h = node_seed(FACE ^ axis as u32, (PIPE, lower));
    let mut p = [mid; 3];
    let mut n = 0;
    let mut j = 0;
    while j < 3 {
        if j == axis {
            p[j] = if up { 8 * 16 } else { 0 };
        } else {
            p[j] = mid + ((h >> (16 * n)) & 0x3f) as i32 - 32;
            n += 1;
        }
        j += 1;
    }
    p
}

/// The squared distance, in sixteenths, from `p` to the segment `a`–`b`.
fn to_segment(p: [i32; 3], a: [i32; 3], b: [i32; 3]) -> i32 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let l = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2];
    let t = if l == 0 || t <= 0 { 0 } else if t >= l { l } else { t };
    let mut d = 0;
    let mut j = 0;
    while j < 3 {
        let q = if l == 0 { ap[j] } else { ap[j] - ab[j] * t / l };
        d += q * q;
        j += 1;
    }
    d
}

/// How near block `b` (0..64) of the pipe cell lies to its piece's line,
/// in from its way in, by its middle, out its way out, nudged by its own
/// hash: the lower, the sooner hollowed.
fn nearness(cell: [i32; 3], p: &Piece, b: u32) -> i32 {
    let q = [(b & 3) as i32 * 32 + 16, (b >> 2 & 3) as i32 * 32 + 16, (b >> 4 & 3) as i32 * 32 + 16];
    let a = crossing(cell, p.from);
    let z = crossing(cell, p.to);
    let m = [(a[0] + z[0]) / 4 + 32, (a[1] + z[1]) / 4 + 32, (a[2] + z[2]) / 4 + 32];
    let d1 = to_segment(q, a, m);
    let d2 = to_segment(q, m, z);
    let d = if d1 < d2 { d1 } else { d2 };
    let g = [cell[0] * 4 + (b & 3) as i32, cell[1] * 4 + (b >> 2 & 3) as i32, cell[2] * 4 + (b >> 4 & 3) as i32];
    d + (node_seed(BLOCKS, (BLOCK, g)) % NUDGE) as i32
}

/// Whether block `b` of the pipe cell is hollow: among the `blocks` nearest.
fn block_hollow(cell: [i32; 3], p: &Piece, b: u32) -> bool {
    if p.blocks == 0 {
        return false;
    }
    if p.blocks >= CELL_BLOCKS {
        return true;
    }
    let mine = nearness(cell, p, b);
    let mut before = 0;
    let mut o = 0;
    while o < CELL_BLOCKS {
        let d = nearness(cell, p, o);
        if d < mine || d == mine && o < b {
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
        while k < 4 {
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
    let mut n = 0;
    let mut z = 0;
    while z < side {
        let mut y = 0;
        while y < side {
            let mut x = 0;
            while x < side {
                let b = (lo[0] + x) as u32 | ((lo[1] + y) as u32) << 2 | ((lo[2] + z) as u32) << 4;
                n += block_hollow(cell, &p, b) as u32;
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
pub fn hollow(node: (Zoom, [i32; 3])) -> u32 {
    let (zoom, c) = node;
    // Coarser than a root, a root's: every root holds the same.
    let (zoom, n) = if zoom < ROOT { (ROOT, blocks((ROOT, [0; 3]))) } else { (zoom, blocks((zoom, c))) };
    // A block is 8 leaves; FULL is 2^15; the node holds 2^(3(LEAF − zoom)) leaves.
    let shift = 3 + 15 - 3 * (LEAF_CELL - zoom) as i32;
    if shift >= 0 {
        let v = n << shift;
        if v > FULL { FULL } else { v }
    } else {
        n >> -shift
    }
}

#[cfg(test)]
#[path = "cave.test.rs"]
mod test;
