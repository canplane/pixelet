// pxl-shader/eye.rs

//! The eye a cast is from, as the kernel writes its words (pxl-engine
//! `eye`), what every entry reads off it, and the eye as an observer
//! (observer.rs `Observer`): its camera grid's corners, cells and samples
//! (pxl-engine `beam::Grid`), a ray at the screen's tangents.

use pxl_engine::cast::{self, Detail};
use pxl_engine::{beam, eye, pyramid, tally};
use spirv_std::glam::{IVec4, UVec2, UVec4, Vec2, Vec4};

use crate::beam::Words;
use crate::observer::{Aim, Level, Observer};

/// The eye a cast is from (pxl-kernel domain/view/lens.rs `gpu_cast`), its words
/// where pxl-engine `eye` lays them (asserted below).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Eye {
    /// The Q3 position, and whether a ray reports the nodes its descent
    /// asked (`word`), not what it met.
    pub origin: IVec4,
    /// The unit forward, and how far the rays go (pxl-engine `eye::FAR`).
    pub forward_far: Vec4,
    /// The unit right, and a pad.
    pub right: Vec4,
    /// The unit up, and a pad.
    pub up: Vec4,
    /// The tangents of the screen's half-angles across and up.
    pub tan: Vec2,
    /// The zooms a cell is asked at. [naga] A vec2 of u32s, which naga's
    /// SPIR-V reader looks for, and never makes, where it splits the layer
    /// off an arrayed image's coordinate (`Rays`).
    pub range: UVec2,
    /// A preview's cut to air and its left-out leaf: X, Y, Z and the zoom,
    /// `eye::NO_CUT` for none.
    pub carve: IVec4,
    pub hide: IVec4,
    /// The light's cast, a frame's constants (pxl-engine `light::Light`), all
    /// zero where no shadow is cast.
    pub light: [Vec4; 5],
    /// Each pixel's ray's offset from its middle, in drawn pixels, and the
    /// first pixel of the region the frame casts (pxl-engine
    /// `eye::JITTER`).
    pub jitter: Vec4,
    /// The camera grid the frame casts on, as the kernel laid it once
    /// (pxl-engine `beam::Grid`, `grid`).
    pub grid: [UVec4; eye::GRID_WORDS / 4],
}

/// Word `words`'s byte.
pub(crate) const fn at_word(words: usize) -> usize {
    words * core::mem::size_of::<u32>()
}

const _: () = {
    use at_word as at;
    use core::mem::{offset_of, size_of};
    assert!(size_of::<Eye>() == at(eye::WORDS));
    assert!(
        offset_of!(Eye, origin) == at(eye::ORIGIN)
            && offset_of!(Eye, forward_far) == at(eye::FORWARD)
            && offset_of!(Eye, right) == at(eye::RIGHT)
    );
    assert!(
        offset_of!(Eye, up) == at(eye::UP)
            && offset_of!(Eye, tan) == at(eye::TAN)
            && offset_of!(Eye, range) == at(eye::RANGE)
    );
    assert!(
        offset_of!(Eye, carve) == at(eye::CARVE)
            && offset_of!(Eye, hide) == at(eye::HIDE)
            && offset_of!(Eye, light) == at(eye::LIGHT)
            && offset_of!(Eye, jitter) == at(eye::JITTER)
            && offset_of!(Eye, grid) == at(eye::GRID)
    );
};

/// Where the eye's rays start, and the zooms each distance asks.
pub(crate) fn eye_origin(eye: &Eye) -> ([i32; 3], Detail) {
    (
        [eye.origin.x, eye.origin.y, eye.origin.z],
        Detail {
            range: [eye.range.x, eye.range.y],
            // The beam rays' lattice's side, the grid's first word
            // (`beam::Grid::words`), which their `narrow` reads.
            spacing: cast::Spacing {
                angle: f32::from_bits(eye.grid[0].x),
                across: 0.0,
            },
        },
    )
}

/// The eye's ray at the tangents `[tx, ty]`, reaching as far as the eye.
pub(crate) fn eye_ray(eye: &Eye, [tx, ty]: [f32; 2]) -> cast::Ray {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    cast::Ray::new(
        cast::through(v3(eye.forward_far), v3(eye.right), v3(eye.up), tx, ty),
        eye.forward_far.w,
    )
}

/// The camera grid the frame casts on, read whole off the eye's words, as
/// the kernel worked it out once (pxl-engine `beam::Grid`).
pub(crate) fn grid(eye: &Eye) -> beam::Grid {
    let g = &eye.grid;
    let w = [
        g[0].x, g[0].y, g[0].z, g[0].w, g[1].x, g[1].y, g[1].z, g[1].w, g[2].x, g[2].y, g[2].z,
        g[2].w, g[3].x, g[3].y, g[3].z, g[3].w, g[4].x, g[4].y, g[4].z, g[4].w,
    ];
    beam::Grid::of_words([eye.tan.x, eye.tan.y], w)
}
const _: () = assert!(eye::GRID_WORDS == 20, "`grid` reads the grid's five vec4s");

/// What the walk over the observed pyramid alone left at the place of the thread
/// `id`, workgroups in a row, off its count and list at word `at`: none past them.
fn left_at<W: Words + ?Sized>(w: &W, at: u32, id: [u32; 2]) -> [u32; 2] {
    let place = (id[0] / 8) * 64 + (id[1] % 8) * 8 + id[0] % 8;
    if place >= w.word(at) {
        return [u32::MAX; 2];
    }
    let s = w.word(at + 1 + place);
    [s & 0xffff, s >> 16]
}

impl Observer for Eye {
    type At = [f32; 2];

    fn tally(&self) -> u32 {
        tally::EYE
    }

    fn assets(&self) -> bool {
        true
    }

    fn passes(&self) -> [u32; 3] {
        [
            pyramid::BEAM_PASS,
            pyramid::SAMPLES_PASS,
            pyramid::MISSED_PASS,
        ]
    }

    fn apart(&self) -> bool {
        false
    }

    fn corners(&self) -> [u32; 2] {
        grid(self).corners
    }

    fn corner(&self, c: [u32; 2]) -> Aim {
        self.aim(grid(self).corner(c))
    }

    fn handed(&self, _: &Aim, t: f32) -> f32 {
        grid(self).hand_dist(t)
    }

    fn line(&self, idx: u32) -> u32 {
        beam::Grid::line(idx)
    }

    fn hand_at(&self, idx: u32) -> u32 {
        grid(self).hand_at(idx)
    }

    /// Every ray of the eye's starts at the eye.
    fn origin_of<W: Words + ?Sized>(&self, _: &W, _: u32) -> [i32; 3] {
        eye_origin(self).0
    }

    fn keep_origin<W: Words + ?Sized>(&self, _: &mut W, _: u32, _: [i32; 3]) {}

    fn cells(&self) -> [u32; 2] {
        grid(self).span
    }

    /// Its own window of corners (`beam::Grid::cell_window`), wider toward
    /// the screen's edge.
    fn window(&self, c: [u32; 2]) -> ([u32; 2], u32) {
        let g = grid(self);
        let k = g.cell_window(c);
        let [x, y] = g.first(c);
        ([x - k, y - k], 2 + 2 * k)
    }

    fn cell_at(&self, c: [u32; 2]) -> u32 {
        grid(self).cell_at(c)
    }

    /// Its own everywhere (`detail`).
    fn corner_detail(&self, _: [u32; 2]) -> Detail {
        eye_origin(self).1
    }

    fn cell_detail(&self, _: [u32; 2]) -> Detail {
        eye_origin(self).1
    }

    fn samples(&self) -> [u32; 2] {
        grid(self).samples
    }

    fn point(&self, s: [u32; 2]) -> [f32; 2] {
        grid(self).sample(s)
    }

    fn sample_cell(&self, s: [u32; 2]) -> [u32; 2] {
        grid(self).sample_cell(s)
    }

    fn sample_at(&self, s: [u32; 2]) -> u32 {
        grid(self).sample_at(s)
    }

    /// Always about it: the lattice runs `param::beam::NEAR` samples past the
    /// screen.
    fn sample_before(&self, at: [f32; 2]) -> ([u32; 2], bool) {
        (grid(self).sample_before(at), true)
    }

    fn aim(&self, at: [f32; 2]) -> Aim {
        let (origin, detail) = eye_origin(self);
        Aim {
            origin,
            ray: eye_ray(self, at),
            detail,
        }
    }

    fn cell_of(&self, at: [f32; 2]) -> [u32; 2] {
        grid(self).cell(at)
    }

    /// Its own everywhere, each distance's zoom from the eye.
    fn detail(&self, _: [f32; 2]) -> Detail {
        eye_origin(self).1
    }

    /// From the eye, its boxes about it marked at the screen's tangents.
    fn raster_layout(&self) -> ([i32; 3], (bool, [f32; 2])) {
        (eye_origin(self).0, (true, [self.tan.x, self.tan.y]))
    }

    /// Every one: what the eye casts is what it shows.
    fn needs<W: Words + ?Sized>(&self, _: &W, _: Level, _: [u32; 2]) -> bool {
        true
    }

    /// Each at its own place: it casts its lattice whole.
    fn take<W: Words + ?Sized>(&self, _: &W, _: Level, id: [u32; 2]) -> [u32; 2] {
        id
    }

    /// The one the walk over the observed pyramid alone left at the thread's
    /// place among them (pxl-render sample_hit.wgsl).
    fn sample_walked<W: Words + ?Sized>(&self, w: &W, id: [u32; 2]) -> [u32; 2] {
        left_at(w, grid(self).lefts_at, id)
    }

    /// As `sample_walked` (pxl-render beam_hit.wgsl).
    fn corner_walked<W: Words + ?Sized>(&self, w: &W, id: [u32; 2]) -> [u32; 2] {
        let g = grid(self);
        // The second round's: what the first left and it did not end.
        left_at(w, g.corners_at + 1 + g.corners[0] * g.corners[1], id)
    }
}
