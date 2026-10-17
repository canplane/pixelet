// pxl-render/frame/shader/floor.rs

//! What every WebGPU device binds, the platform's floor: each limit's
//! default (WebGPU, "Limits", https://www.w3.org/TR/webgpu/#limit-default),
//! which a device past it is no target for, so no pipeline layout binds more
//! (`holds`). Counted as the device counts them at a pipeline layout: per
//! stage, over every group, of what each group's layout makes visible to it.

use crate::frame::shader::record::{
    COMPUTE, DEPTH, FRAGMENT, SAMPLER, STORAGE, STORAGE_RW, STORAGE_TEXTURE_RW, TEXTURE, TEXTURE_ARRAY, TEXTURE_UINT_ARRAY, TEXTURE_UNFILTERED, UNIFORM, UNIFORM_DYNAMIC, VERTEX,
};

pub const BIND_GROUPS: usize = 4;
pub const BINDINGS_PER_BIND_GROUP: u32 = 1000;
pub const STORAGE_BUFFERS_PER_STAGE: u32 = 8;
pub const UNIFORM_BUFFERS_PER_STAGE: u32 = 12;
pub const STORAGE_TEXTURES_PER_STAGE: u32 = 4;
pub const SAMPLED_TEXTURES_PER_STAGE: u32 = 16;
pub const SAMPLERS_PER_STAGE: u32 = 16;
pub const DYNAMIC_UNIFORM_BUFFERS_PER_LAYOUT: u32 = 8;

/// Whether a pipeline layout of `groups`, each a group's (binding, stages,
/// kind) words as record.rs and build.rs `compute` state them, binds within
/// the floor: its groups, each group's bindings, and per stage each kind of
/// binding over all of them.
pub const fn holds(groups: &[&[u32]]) -> bool {
    if groups.len() > BIND_GROUPS {
        return false;
    }
    let stages = [VERTEX, FRAGMENT, COMPUTE];
    let mut dynamic = 0;
    let mut s = 0;
    while s < stages.len() {
        // Storage buffers, uniform buffers, storage textures, sampled
        // textures, samplers.
        let mut count = [0u32; 5];
        let mut g = 0;
        while g < groups.len() {
            let words = groups[g];
            let mut k = 0;
            while k + 2 < words.len() {
                let (binding, visible, kind) = (words[k], words[k + 1], words[k + 2]);
                if binding >= BINDINGS_PER_BIND_GROUP {
                    return false;
                }
                if s == 0 && kind == UNIFORM_DYNAMIC {
                    dynamic += 1;
                }
                // A kind not named here is one the floor cannot count: no
                // layout holding it holds.
                let class = match kind {
                    STORAGE | STORAGE_RW => 0,
                    UNIFORM | UNIFORM_DYNAMIC => 1,
                    STORAGE_TEXTURE_RW => 2,
                    TEXTURE | TEXTURE_ARRAY | DEPTH | TEXTURE_UNFILTERED | TEXTURE_UINT_ARRAY => 3,
                    SAMPLER => 4,
                    _ => return false,
                };
                if visible & stages[s] != 0 {
                    count[class] += 1;
                }
                k += 3;
            }
            g += 1;
        }
        if count[0] > STORAGE_BUFFERS_PER_STAGE
            || count[1] > UNIFORM_BUFFERS_PER_STAGE
            || count[2] > STORAGE_TEXTURES_PER_STAGE
            || count[3] > SAMPLED_TEXTURES_PER_STAGE
            || count[4] > SAMPLERS_PER_STAGE
        {
            return false;
        }
        s += 1;
    }
    dynamic <= DYNAMIC_UNIFORM_BUFFERS_PER_LAYOUT
}
