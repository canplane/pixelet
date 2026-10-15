// pxl-render/gpu/shadow.rs

//! The sun's shadow as the lit passes bind it: a blank map until the sun's
//! rays are cast (the screen's cast), so every lit face reads it unshadowed.

use crate::gpu::api::{object, texture_usage, Gpu};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, resource};

pub struct Shadow {
    view: u32,
    sampler: u32,
}

impl Shadow {
    pub fn new(g: &Gpu) -> Shadow {
        let blank = object();
        g.texture(blank, 1, 1, 1, format::DEPTH32, 1, texture_usage::TEXTURE_BINDING);
        let view = object();
        g.view(view, blank, dimension::D2, 0, 1, 0, 1);
        let sampler = object();
        g.sampler(sampler, filter::NEAREST, filter::NEAREST, filter::NEAREST, address::CLAMP, compare::LESS_EQUAL, 1);
        Shadow { view, sampler }
    }

    pub fn entries(&self, binding: u32) -> [u32; 10] {
        [binding, resource::VIEW, self.view, 0, 0, binding + 1, resource::SAMPLER, self.sampler, 0, 0]
    }
}
