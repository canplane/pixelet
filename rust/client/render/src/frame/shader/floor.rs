// pxl-render/frame/shader/floor.rs

//! The floor: the default of each WebGPU limit (WebGPU, "Limits",
//! https://www.w3.org/TR/webgpu/#limit-default). Every WebGPU device
//! supports at least these values, so the renderer stays within them: no
//! pipeline layout binds more than they allow (`holds`), and no buffer is
//! larger than they allow (`fits`). A device counts a layout's
//! bindings per stage, over all its groups, and in each group counts only
//! the bindings the group's layout makes visible to that stage; `holds`
//! counts them the same way.

use xpute_gpu_link::spec::shader_stage::{COMPUTE, FRAGMENT, VERTEX};

use crate::api_spec::record::{
    DEPTH, SAMPLER, SAMPLER_COMPARISON, STORAGE, STORAGE_RW, STORAGE_TEXTURE_2D,
    STORAGE_TEXTURE_3D, STORAGE_TEXTURE_RW, TEXTURE, TEXTURE_3D, TEXTURE_ARRAY, TEXTURE_UINT_ARRAY,
    TEXTURE_UNFILTERED, UNIFORM, UNIFORM_DYNAMIC,
};

pub const BIND_GROUPS: usize = 4;
pub const VERTEX_BUFFERS: usize = 8;
pub const BINDINGS_PER_BIND_GROUP: u32 = 1000;
pub const STORAGE_BUFFERS_PER_STAGE: u32 = 8;
pub const UNIFORM_BUFFERS_PER_STAGE: u32 = 12;
pub const STORAGE_TEXTURES_PER_STAGE: u32 = 4;
pub const SAMPLED_TEXTURES_PER_STAGE: u32 = 16;
pub const SAMPLERS_PER_STAGE: u32 = 16;
pub const DYNAMIC_UNIFORM_BUFFERS_PER_LAYOUT: u32 = 8;
pub const MAX_STORAGE_BUFFER_BINDING_SIZE: u64 = 134_217_728;
pub const MAX_BUFFER_SIZE: u64 = 268_435_456;
pub const MAX_TEXTURE_DIMENSION_2D: u32 = 8192;
pub const MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP: u32 = 256;

/// Whether every device can make a buffer of `bytes` and, when `storage` is
/// set, bind it as a storage buffer. The storage limit is checked against
/// the whole buffer, whatever range of it a binding takes.
pub const fn fits(bytes: u64, storage: bool) -> bool {
    bytes <= MAX_BUFFER_SIZE && (!storage || bytes <= MAX_STORAGE_BUFFER_BINDING_SIZE)
}

/// Whether a pipeline layout binds within the floor: its number of groups,
/// the binding numbers in each group, the bindings of each kind per stage
/// over all groups, and its dynamic uniform buffers. Each of `layouts` is one
/// group's entries as triples of words (binding, visible stages, kind), laid
/// out as a pipeline's record lays them out (record.rs).
pub const fn holds(layouts: &[&[u32]]) -> bool {
    if layouts.len() > BIND_GROUPS {
        return false;
    }
    let stages = [VERTEX, FRAGMENT, COMPUTE];
    let mut dynamic = 0;
    let mut s = 0;
    while s < stages.len() {
        // One count for each class of binding: storage buffers, uniform
        // buffers, storage textures, sampled textures and samplers.
        let mut cnt = [0u32; 5];
        let mut g = 0;
        while g < layouts.len() {
            let words = layouts[g];
            let mut k = 0;
            while k + 2 < words.len() {
                let (binding, visible, kind) = (words[k], words[k + 1], words[k + 2]);
                if binding >= BINDINGS_PER_BIND_GROUP {
                    return false;
                }
                if s == 0 && kind == UNIFORM_DYNAMIC {
                    dynamic += 1;
                }
                // A kind not listed here has no limit to be counted
                // against, so a layout that holds one fails.
                let class = match kind {
                    STORAGE | STORAGE_RW => 0,
                    UNIFORM | UNIFORM_DYNAMIC => 1,
                    STORAGE_TEXTURE_RW | STORAGE_TEXTURE_3D | STORAGE_TEXTURE_2D => 2,
                    TEXTURE | TEXTURE_ARRAY | DEPTH | TEXTURE_UNFILTERED | TEXTURE_UINT_ARRAY
                    | TEXTURE_3D => 3,
                    SAMPLER | SAMPLER_COMPARISON => 4,
                    _ => return false,
                };
                if visible & stages[s] != 0 {
                    cnt[class] += 1;
                }
                k += 3;
            }
            g += 1;
        }
        if cnt[0] > STORAGE_BUFFERS_PER_STAGE
            || cnt[1] > UNIFORM_BUFFERS_PER_STAGE
            || cnt[2] > STORAGE_TEXTURES_PER_STAGE
            || cnt[3] > SAMPLED_TEXTURES_PER_STAGE
            || cnt[4] > SAMPLERS_PER_STAGE
        {
            return false;
        }
        s += 1;
    }
    dynamic <= DYNAMIC_UNIFORM_BUFFERS_PER_LAYOUT
}
