// pxl-kernel/scene/decor/cell.rs

//! A block of instance lanes drawn by size: each instance a model, a sprite
//! or nothing by the pixels it spans, decided per frame and rewriting only
//! its index list (`present_block`). Clouds are drawn so; the GPU draws decor
//! by the same tiers, one rule (pxl-spec-core scatter.rs `drawn::tier`).

use pxl_render::frame::draw::{DrawList, PIPELINE_DECOR, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR};
use pxl_spec::space::coord::v3::{self, V3};
use xpute_guest::mem::base::ptr_at;

use pxl_spec_core::scatter::drawn;

use crate::scene::decor::drawn::{decor_gone_px, decor_sprite_px};
use crate::scene::decor::instance::{decor_groups, GroupLanes};
use crate::view::frame::pose::units_per_px_at;

/// A block of decor lanes as `present_block` reads it: its square's corner
/// about the eye's lap, its side, and the heights its instances stand between.
pub struct Shown {
    pub handle: u32,
    pub at: u32,
    pub index: u32,
    pub bytes: u32,
    pub corner: [f64; 2],
    pub side: f64,
    pub heights: [f32; 2],
}

/// Lists one block's instances, model, sprite or nothing each, under the
/// draw groups `groups` names in the block's order, each with its model's
/// extent and the least and most scale its instances are drawn at. Under an
/// `opacity` of one the models are laid over the world blended, and a sprite,
/// which cannot be, is left out.
#[allow(clippy::too_many_arguments)]
pub fn present_block(draw: &mut DrawList, b: &Shown, translation: [f32; 3], eye: V3, groups: impl Iterator<Item = (u32, (f64, [f64; 2]))>, opacity: f32, sprites: &mut Vec<u32>) {
    let (model_px, sprite_px) = (decor_gone_px() as f32, decor_sprite_px() as f32);
    let tier = |px: f64| drawn::tier(px as f32, model_px, sprite_px);
    let [x0, y0] = b.corner;
    let [zlo, zhi] = b.heights.map(f64::from);
    let gap = |lo: f64, hi: f64, v: f64| (lo - v).max(v - hi).max(0.0);
    let reach = |lo: f64, hi: f64, v: f64| (v - lo).abs().max((hi - v).abs());
    let nearest = v3::len(&[gap(x0, x0 + b.side, eye[0]), gap(y0, y0 + b.side, eye[1]), gap(zlo, zhi, eye[2])]).max(1e-3);
    let farthest = v3::len(&[reach(x0, x0 + b.side, eye[0]), reach(y0, y0 + b.side, eye[1]), reach(zlo, zhi, eye[2])]).max(1e-3);
    // SAFETY: the block, `bytes` long, its owner's to write past `index`.
    let words: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(ptr_at::<u32>(b.at as usize), b.bytes as usize / 4) };
    let (lanes_of, list) = words.split_at_mut(b.index as usize / 4);
    let (mut at, mut changed) = (0usize, false);
    for ((offset, count), (group, (extent, [least, most]))) in decor_groups(lanes_of).zip(groups) {
        let lane = lanes(offset, count);
        let first = at;
        sprites.clear();
        let gone = tier(extent * most / units_per_px_at(nearest)) == drawn::GONE;
        let modeled = tier(extent * least / units_per_px_at(farthest)) == drawn::MODEL;
        for i in 0..if gone { 0 } else { count as usize } {
            if (lanes_of[lane.flags()..][i / 4] >> ((i % 4) * 8)) & 1 != 0 {
                continue;
            }
            if modeled {
                changed |= list[at] != i as u32;
                list[at] = i as u32;
                at += 1;
                continue;
            }
            let f = |l: usize| f32::from_bits(lanes_of[lane.f32_at(l, i)]) as f64;
            let d = v3::len(&[x0 + f(0) - eye[0], y0 + f(1) - eye[1], f(2) - eye[2]]).max(1e-3);
            let t = tier(extent * f(4) / units_per_px_at(d));
            if t == drawn::MODEL {
                changed |= list[at] != i as u32;
                list[at] = i as u32;
                at += 1;
            } else if t == drawn::SPRITE && opacity >= 1.0 {
                sprites.push(i as u32);
            }
        }
        let models = (at - first) as u32;
        changed |= list[at..at + sprites.len()] != sprites[..];
        list[at..at + sprites.len()].copy_from_slice(sprites);
        at += sprites.len();
        let index = b.index / 4 + first as u32;
        if models > 0 {
            let pipeline = if opacity < 1.0 { PIPELINE_GHOST_DECOR } else { PIPELINE_DECOR };
            draw.draw_instances(pipeline, b.handle, group, models, [offset, count, index], translation, opacity);
        }
        if !sprites.is_empty() {
            draw.draw_instances(PIPELINE_IMPOSTOR, b.handle, group, sprites.len() as u32, [offset, count, index + models], translation, 1.0);
        }
    }
    if changed {
        draw.rewrite_part(b.handle, b.at, b.index, at as u32 * 4);
    }
}

pub fn lanes(offset: u32, count: u32) -> GroupLanes {
    GroupLanes {
        first: offset as usize,
        count: count as usize,
    }
}
