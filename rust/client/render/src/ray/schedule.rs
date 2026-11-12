// pxl-render/ray/schedule.rs

//! Decides what each frame casts of the eye's rays and the light's. It is a pure state
//! machine: `Schedule::plan` steps it once a frame from what the frame read (`CastRead`), and
//! `whole`, `dropped` and `off` step it by what befell the frame after. It holds no GPU and no
//! clock, so its tests drive it on the CPU alone (schedule.test.rs). The cast (cast.rs
//! `prepare`) reads the frame, asks for a plan and encodes what the plan says.

use super::casts_px;
use crate::gpu::span_spec::{FrameKind, LightChanged};

/// What a frame read that decides what the eye and the light cast.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CastRead {
    /// Whether the cast's input is byte for byte the last frame's and the cast's group is made
    /// (cast.rs `prepare`). The input is the eye's words without the light's, the jitter's and
    /// the grid's; the clip levels' table; the shards; the objects' room and revision; and
    /// the world surface.
    pub same: bool,
    /// Where each pixel's ray passes through its pixel this frame (pxl-engine `eye::JITTER`).
    /// The jitter steps through its sequence only while a change other than the scene's
    /// animation settles (pass/resolve.rs `asks`), so a frame that only animates casts no pixel
    /// again.
    pub jitter: [u32; 2],
    /// Whether nothing moved but the listed draws that mark tiles, so the frame may be drawn
    /// through the gate in the damaged tiles alone (renderer.rs `partial`).
    pub partial: bool,
    /// Whether shadows are cast, and whether the light's cast has its buffers (cast.rs
    /// `light_made`).
    pub shadows: bool,
    pub light_made: bool,
    /// Whether what decides a shadow map tile's content reads as the light's last cast read it
    /// (cast.rs `light_content`): the light's frame, whether shadows are on, and whether the
    /// world under the map moved whole.
    pub content_same: bool,
    /// Whether the world under the map moved only in columns the cast can name. The epoch then
    /// goes on, and only the tiles those columns reach are cast again.
    pub world_columns: bool,
    /// The light box's corner (pxl-engine `light::CORNER`), and the eye's zooms
    /// (`eye::RANGE`), which bound each tile's band (`Light::band`).
    pub corner: [i32; 2],
    pub range: [u32; 2],
    /// Whether what decides which tiles the receivers read reads as last time (cast.rs
    /// `light_receiver_read`). The host's draws' boxes are taken with their footprints where the
    /// eye stands (`Schedule::stands`).
    pub receivers_same: bool,
}

/// What the light's cast does in a frame that casts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Light {
    /// What changed that it casts, a bit for each `LightChanged` (span.json `LIGHT_CHANGED`).
    /// It is never 0.
    pub changed: u32,
    /// Whether an epoch begins (pxl-engine `light::HDR_EPOCH`). An epoch is a run of casts
    /// over which what decides a tile's content stands, so a tile one of them cast stands for
    /// the rest; it begins where that content moved.
    pub epoch: bool,
    /// Whether the box slid or the eye's zooms moved within the epoch. The reads then move with
    /// the box, and a tile whose band moved is cast again (`light_slide_cs`).
    pub slid: bool,
    /// Whether columns of the world changed within the epoch, so the tiles whose rays may meet
    /// them are cast again.
    pub columns_changed: bool,
    /// The box's corner this cast is cast under, and the corner of the cast before
    /// (`light::HDR_LAST_CORNER`).
    pub corner: [i32; 2],
    pub last_corner: [i32; 2],
    /// The eye's zooms the cast before was cast under (`light::HDR_LAST_RANGE`).
    pub last_range: [u32; 2],
    /// Whether this cast marks the tiles the world's receivers stand on
    /// (`light::HDR_WORLD_RECEIVERS`). A receiver is a drawn thing that reads the shadow
    /// map; the world's receivers are the voxels the eye's rays hit.
    pub world: bool,
    /// Whether those marks are added to the ones already made, as the walked bits cover the
    /// damaged tiles alone. Otherwise the marks start afresh.
    pub additive: bool,
    /// Whether the receivers' marks take each pixel's footprint, which they do while the eye
    /// stands (`light::HDR_FOOTPRINT`).
    pub footprint: bool,
}

/// What a frame casts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Plan {
    /// `plain` casts nothing, as there is no world to cast in; `still` keeps the rays as they
    /// are; `px` casts the pixels again over the samples that stand; `cast` runs the whole cast.
    pub kind: FrameKind,
    /// Whether every pixel is cast again though only the damaged tiles moved. The light's cast
    /// needs walked bits that cover every pixel where it marks the world's receivers afresh,
    /// and a gated cast would leave them covering the damaged tiles alone.
    pub unmasked: bool,
    /// What the light's cast does, or none where it does not cast.
    pub light: Option<Light>,
}

impl Plan {
    const PLAIN: Plan = Plan {
        kind: FrameKind::plain,
        unmasked: false,
        light: None,
    };

    pub fn casts(&self) -> bool {
        casts_px(self.kind)
    }
}

/// The state one frame's plan leaves for the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Schedule {
    /// Whether the rays hold what the cast's input last read: the last frame read it, had a
    /// world to cast in, and was not dropped.
    read: bool,
    /// The jitter the last frame read.
    jitter: [u32; 2],
    /// Whether the eye's walked bits cover every drawn pixel. The walked bits mark the holes,
    /// the pixels the raster left no voxel at, whose rays the holes' queue walks apart
    /// (pxl-engine `ray::SQUARE_WORDS`). A gated pixels' cast sets them in the damaged tiles
    /// alone.
    walked_whole: bool,
    /// Whether the shadow map's tiles hold what the light's content last read: the last frame
    /// read it with the light's cast made, and was not dropped.
    content: bool,
    /// The box's corner and the eye's zooms the light last cast under.
    corner: [i32; 2],
    range: [u32; 2],
    /// Whether the next light cast marks the tiles the world's receivers stand on, from what
    /// the eye's rays hit.
    world: bool,
    /// Whether the marks with each pixel's footprint are owed: the last cast that marked the
    /// world's receivers did so while the eye moved, or for the damaged tiles alone. They fall
    /// due once the eye stands.
    footprint_owed: bool,
    /// The last frame's plan.
    plan: Plan,
}

impl Schedule {
    pub const fn new() -> Schedule {
        Schedule {
            read: false,
            jitter: [0; 2],
            walked_whole: false,
            content: false,
            corner: [0; 2],
            range: [0; 2],
            world: false,
            footprint_owed: false,
            plan: Plan::PLAIN,
        }
    }

    /// Whether the view and the world stand: the cast's input is the last frame's
    /// (`CastRead::same`), and the rays hold what it read.
    pub fn stands(&self, same: bool) -> bool {
        same && self.read
    }

    /// Steps the schedule by what this frame read, and returns the frame's plan.
    ///
    /// The eye casts whole where the view or the world moved. While they stand, it casts the
    /// pixels again over the samples that stand where the jitter moved, and nothing otherwise.
    ///
    /// The light casts while shadows are on and its cast is made, where what a tile holds
    /// moved, the box slid, which tiles are read moved, the footprints are owed, or the world
    /// moved in columns.
    pub fn plan(&mut self, r: CastRead) -> Plan {
        let stands = self.stands(r.same);
        self.read = true;
        let jittered = r.jitter != self.jitter;
        self.jitter = r.jitter;
        let mut kind = match (stands, jittered) {
            (true, false) => FrameKind::still,
            (true, true) => FrameKind::px,
            (false, _) => FrameKind::cast,
        };
        let light_on = r.shadows && r.light_made;
        let unchanged = r.content_same && self.content;
        self.content = r.light_made;
        // A step of the eye's zooms changes what a tile holds only where it moves the tile's
        // band, as a slide does, so it is handled as a slide (`Light::keeps`).
        let slid = r.corner != self.corner || r.range != self.range;
        // The first frame the eye stands in after it moved marks the receivers again, with the
        // footprints that cover where the jittered rays of the frames after it land.
        let footprint_owed = self.footprint_owed && stands;
        // The world's receivers are marked afresh where the light's content or the box moved,
        // and a fresh marking reads every pixel's walk. The owed footprints read every pixel's
        // walk too, as the pixels the walked bits leave out still owe theirs. Where the walked
        // bits would cover the damaged tiles alone, because the last pixels' cast or this
        // frame's is gated, every pixel is cast again now (`unmasked`), so the bits cover every
        // pixel. The marking then follows the bits alone (`walked_whole`): it starts afresh
        // where they cover every pixel, and is added to the marks already made otherwise, the
        // light and the box being as they were.
        let partial = if casts_px(kind) {
            r.partial && kind == FrameKind::px
        } else {
            !self.walked_whole
        };
        let unmasked = light_on && (!unchanged || slid || footprint_owed) && partial;
        if unmasked && kind == FrameKind::still {
            kind = FrameKind::px;
        }
        let casts = casts_px(kind);
        let walked_whole = !partial || unmasked;
        let changed = LightChanged::ALL
            .into_iter()
            .filter(|m| match m {
                LightChanged::content => !unchanged,
                LightChanged::slide => slid,
                LightChanged::receivers => !r.receivers_same,
                LightChanged::footprint => footprint_owed,
                LightChanged::world => r.world_columns,
            })
            .fold(0, |bits, m| bits | 1 << m as u32);
        // The world's receivers stand where the eye's rays hit. The next light cast marks them
        // again after the rays were cast anew, where an epoch begins, where the footprints are
        // owed, or where the box slid, which moves the tiles under them. Otherwise they stand
        // on the tiles they last marked: a jittered cast moves a ray only within its footprint,
        // which those marks took where the eye stood.
        self.world |= kind == FrameKind::cast || !unchanged || footprint_owed || slid;
        let light = (light_on && changed != 0).then(|| {
            let last_corner = core::mem::replace(&mut self.corner, r.corner);
            let last_range = core::mem::replace(&mut self.range, r.range);
            // The marks are added to those already made where the walked bits cover the
            // damaged tiles alone, which happens only within the epoch and under the same box.
            let additive = !walked_whole;
            xpute_kit::ensure!(!additive || (unchanged && !slid), ENOTRECOVERABLE);
            let world = core::mem::take(&mut self.world);
            // Marks added for the damaged tiles alone leave the footprint owed to the rest, as
            // marks made while the eye moves do.
            if world {
                self.footprint_owed = !stands || additive;
            }
            Light {
                changed,
                epoch: !unchanged,
                slid: unchanged && slid,
                columns_changed: unchanged && r.world_columns,
                corner: r.corner,
                last_corner,
                last_range,
                world,
                additive: world && additive,
                footprint: stands,
            }
        });
        if casts {
            self.walked_whole = !(r.partial && kind == FrameKind::px && !unmasked);
        }
        self.plan = Plan {
            kind,
            unmasked,
            light,
        };
        self.plan
    }

    /// The renderer draws this frame whole after all. Its samples' raster is then drawn whole,
    /// and where it casts the pixels, its walked bits cover every pixel.
    pub fn whole(&mut self) {
        if self.plan.casts() {
            self.walked_whole = true;
        }
    }

    /// The frame was dropped with its cast: the rays and the map hold older ones, so the next
    /// frame casts both afresh.
    pub fn dropped(&mut self) {
        (self.read, self.content) = (false, false);
    }

    /// There is no world to cast in. The frame casts nothing, and neither the rays nor the map
    /// hold what was read.
    pub fn off(&mut self) {
        (self.read, self.content) = (false, false);
        self.plan = Plan::PLAIN;
    }

    /// The last frame's plan.
    pub fn last(&self) -> &Plan {
        &self.plan
    }

    /// Whether the samples' raster is drawn through its gate, in the damaged tiles alone. It is
    /// where the frame casts the pixels and its walked bits cover those tiles alone.
    pub fn masked(&self) -> bool {
        self.plan.casts() && !self.walked_whole
    }
}

#[cfg(test)]
#[path = "schedule.test.rs"]
mod test;
