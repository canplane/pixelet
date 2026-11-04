// pxl-shader/eye.rs

//! The eye a cast is from, as the kernel writes its words (pxl-engine
//! `eye`), what every entry reads off it, and the eye as an observer
//! (observer.rs `Observer`): its camera grid's corners, cells and samples
//! (pxl-engine `beam::Grid`), a ray at the screen's tangents.

use pxl_engine::cast::{self, Detail};
use pxl_engine::{beam, eye};
use spirv_std::glam::{IVec4, UVec2, UVec4, Vec2, Vec4};

use crate::beam::Words;
use crate::observer::Observer;

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

impl Observer for Eye {
    fn corners(&self) -> [u32; 2] {
        grid(self).corners
    }

    fn hand_at(&self, idx: u32) -> u32 {
        grid(self).hand_at(idx)
    }

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

    /// Every one: what the eye casts is what it shows.
    fn needs<W: Words + ?Sized>(&self, _: &W, _: [u32; 2]) -> bool {
        true
    }

    /// Each at its own place: it casts its lattice whole.
    fn take<W: Words + ?Sized>(&self, _: &W, id: [u32; 2]) -> [u32; 2] {
        id
    }
}
