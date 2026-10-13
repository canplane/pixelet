// pxl-kernel/chunk/table/census.rs

//! What the table counts for the profiler: chunks by state, and what is held.

use crate::profiler::frame::ZOOMS;

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkCounts {
    pub added: u32,
    pub removed: u32,
    pub layouts: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkHeld {
    pub tiers: u32,
}

#[derive(Clone, Copy, Default)]
pub struct Census {
    pub chunks: u32,
    pub in_frustum: u32,
    pub settling: u32,
    /// Pending after the frame, counted directly: "start minus committed" would
    /// miss chunks whose build landed this frame.
    pub pending: u32,
    pub offscreen_pending: u32,
    pub oldest_visible_ms: f64,
    pub oldest_offscreen_ms: f64,
    pub ready_waiting: u32,
    pub slot_waits: u32,
    /// Bit z: a cell of zoom z waited for a slot.
    pub wait_zooms: u32,
    pub chained: u32,
    pub fine_peak: u32,
    pub coarse_peak: u32,
    pub refused: u32,
    pub waiting: u32,
    pub zoom_boxes: [u32; ZOOMS],
    /// Chunks wanting a layout the commit pass did not reach: whether the frame's cap is the bottleneck.
    pub commit_left: u32,
}

impl Scheduler {
    /// The chunks counted for the profiler at `t0`, the slots' waits taken
    /// whether or not they are: a recording starts from now.
    pub fn census(&mut self, counted: bool, t0: f64) -> Census {
        let mut c = Census {
            chunks: self.slots.len() as u32,
            commit_left: self.commit_left,
            refused: self.moves.0,
            waiting: self.moves.1,
            zoom_boxes: self.zoom_boxes,
            slot_waits: core::mem::take(&mut self.meshes.waits),
            wait_zooms: core::mem::take(&mut self.meshes.wait_zooms),
            chained: core::mem::take(&mut self.meshes.chained),
            fine_peak: core::mem::take(&mut self.meshes.fine_peak),
            coarse_peak: core::mem::take(&mut self.meshes.coarse_peak),
            ..Census::default()
        };
        if !counted {
            return c;
        }
        for s in &self.slots {
            c.in_frustum += u32::from(s.in_frustum);
            c.settling += u32::from(s.settling());
            c.pending += u32::from(s.pending());
            c.ready_waiting += u32::from(s.role == Role::Building && s.layout.is_some());
            if let Some(since) = s.pending_since {
                let waited = t0 - since;
                if s.in_frustum {
                    c.oldest_visible_ms = c.oldest_visible_ms.max(waited);
                } else {
                    c.offscreen_pending += 1;
                    c.oldest_offscreen_ms = c.oldest_offscreen_ms.max(waited);
                }
            }
        }
        c
    }

    pub fn take_counts(&mut self) -> ChunkCounts {
        core::mem::take(&mut self.counts)
    }

    pub fn held(&self) -> ChunkHeld {
        let mut out = ChunkHeld::default();
        for s in &self.slots {
            out.tiers += s.tiers.len() as u32;
        }
        out
    }
}
