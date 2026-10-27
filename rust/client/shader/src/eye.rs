// pxl-shader/eye.rs

//! The eye a cast is from, as the kernel writes its words (pxl-spec-core
//! `eye`), what every entry reads off it, and the eye as an observer
//! (observer.rs `Observer`): its camera grid's corners, cells and samples
//! (pxl-spec-core `beam::Grid`), a ray at the screen's tangents.

use pxl_spec_core::cast::{self, Detail};
use pxl_spec_core::{beam, eye, observed, tally};
use spirv_std::glam::{IVec4, UVec2, UVec4, Vec2, Vec4};

use crate::beam::Words;
use crate::observer::{Aim, Level, Observer};

/// The eye a cast is from (pxl-kernel domain/view/lens.rs `gpu_cast`), its words
/// where pxl-spec-core `eye` lays them (asserted below).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Eye {
    /// The Q3 position, and whether a ray reports the nodes its descent
    /// asked (`word`), not what it met.
    pub origin: IVec4,
    /// The unit forward, and the reach.
    pub forward_far: Vec4,
    /// The unit right, and τ.
    pub right_tau: Vec4,
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
    /// The camera grid the frame casts on, as the kernel laid it once
    /// (pxl-spec-core `beam::Grid`, `grid`).
    pub grid: [UVec4; eye::GRID_WORDS / 4],
    /// The sun's cast, a frame's constants (pxl-spec-core `sun::Sun`), all
    /// zero where no shadow is cast.
    pub sun: [Vec4; 5],
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
            && offset_of!(Eye, right_tau) == at(eye::RIGHT)
    );
    assert!(
        offset_of!(Eye, up) == at(eye::UP)
            && offset_of!(Eye, tan) == at(eye::TAN)
            && offset_of!(Eye, range) == at(eye::RANGE)
    );
    assert!(
        offset_of!(Eye, carve) == at(eye::CARVE)
            && offset_of!(Eye, hide) == at(eye::HIDE)
            && offset_of!(Eye, grid) == at(eye::GRID)
            && offset_of!(Eye, sun) == at(eye::SUN)
    );
};

/// Where the eye's rays start, and the zooms each distance asks.
pub(crate) fn eye_origin(eye: &Eye) -> ([i32; 3], Detail) {
    (
        [eye.origin.x, eye.origin.y, eye.origin.z],
        Detail {
            tau: eye.right_tau.w,
            range: [eye.range.x, eye.range.y],
            eye: [0.0; 3],
            cone: cast::Cone {
                angle: pxl_spec_core::beam::spacing(eye.right_tau.w),
                across: 0.0,
            },
            apart: 1.0,
        },
    )
}

/// The eye's ray at the tangents `[tx, ty]`, reaching as far as the eye.
pub(crate) fn eye_ray(eye: &Eye, [tx, ty]: [f32; 2]) -> cast::Ray {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    cast::Ray::new(
        cast::through(v3(eye.forward_far), v3(eye.right_tau), v3(eye.up), tx, ty),
        eye.forward_far.w,
    )
}

/// The camera grid the frame casts on, read whole off the eye's words, as
/// the kernel worked it out once (pxl-spec-core `beam::Grid`): no thread
/// works any of it out again.
pub(crate) fn grid(eye: &Eye) -> beam::Grid {
    let g = &eye.grid;
    let w = [
        g[0].x, g[0].y, g[0].z, g[0].w, g[1].x, g[1].y, g[1].z, g[1].w, g[2].x, g[2].y, g[2].z,
        g[2].w, g[3].x, g[3].y, g[3].z, g[3].w,
    ];
    beam::Grid::of_words([eye.tan.x, eye.tan.y], w)
}
const _: () = assert!(eye::GRID_WORDS == 16, "`grid` reads the grid's four vec4s");

impl Observer for Eye {
    type At = [f32; 2];
    const TALLY: u32 = tally::EYE;
    const ASSETS: bool = true;
    const PASSES: [u32; 3] = [
        observed::BEAM_PASS,
        observed::SAMPLES_PASS,
        observed::MISSED_PASS,
    ];
    const APART: bool = false;

    fn corners(&self) -> [u32; 2] {
        grid(self).corners
    }

    fn corner(&self, c: [u32; 2]) -> Aim {
        self.aim(grid(self).corner(c))
    }

    fn handed(&self, _: &Aim, t: f32) -> f32 {
        grid(self).handed(t)
    }

    fn line(&self, index: u32) -> u32 {
        beam::Grid::line(index)
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

    /// Always about it: the lattice runs `beam::NEAR` samples past the
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

    /// From the eye, its boxes about it marked at the screen's tangents.
    fn laid(&self) -> ([i32; 3], (bool, [f32; 2])) {
        (eye_origin(self).0, (true, [self.tan.x, self.tan.y]))
    }

    /// Every one: what the eye casts is what it shows.
    fn needs<W: Words + ?Sized>(&self, _: &W, _: Level, _: [u32; 2]) -> bool {
        true
    }
}
