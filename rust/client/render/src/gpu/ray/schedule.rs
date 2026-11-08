// pxl-render/gpu/ray/schedule.rs

//! What each frame runs of the eye's rays and the light's: a pure state machine of what the
//! frame read (`Reads`), stepped once a frame (`Schedule::plan`) and by what befalls the frame
//! after (`whole`, `dropped`, `off`, `light_off`), with no GPU and no clock, so every state it
//! reaches is explored whole on the CPU (schedule.test.rs). The cast (cast.rs `prepare`) reads
//! the frame, asks for a plan and writes what the plan says.

use super::casts_px;
use crate::gpu::span_spec::{FrameKind, LightMoved};

/// What a frame read that decides what the eye and the light run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Reads {
    /// Whether the cast's input reads as the last frame's did (cast.rs `prepare`): the eye's
    /// words but the light's, the jitter's and the grid's, the clip levels' table, the shards,
    /// the overwrites' room and revision and the surface, byte for byte; and the cast's group
    /// made.
    pub same: bool,
    /// Where each pixel's ray goes through it (pxl-engine `eye::JITTER`): the world's
    /// sequence, which moves only while a change that is not the scene's animation's settles
    /// (gpu/resolve.rs `asks`). An animation's step holds it, so a turn that only animates
    /// casts no pixel (schedule.test.rs `rests`).
    pub jitter: [u32; 2],
    /// Whether nothing moved but the listed draws that mark (renderer.rs `partial`).
    pub partial: bool,
    /// Whether shadows are cast, and whether the light's cast is made (cast.rs `light_made`).
    pub shadows: bool,
    pub light_made: bool,
    /// Whether what a tile of the shadow map holds reads as the light's last cast read it
    /// (cast.rs `light_content`): the light's frame, the shadows, and the world under it moved
    /// whole or not.
    pub content_same: bool,
    /// Whether the world under the map moved in columns alone, the epoch going on.
    pub world_columns: bool,
    /// The light's box's corner (pxl-engine `light::CORNER`), and the eye's zooms, which its
    /// tiles' bands are held to (pxl-engine `eye::RANGE`, `Light::band`).
    pub corner: [i32; 2],
    pub range: [u32; 2],
    /// Whether which tiles the receivers read reads as last time (cast.rs `light_receivers`),
    /// their boxes taken as the eye stands (`Schedule::stands`).
    pub receivers_same: bool,
}

/// What the light's cast does, where it casts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Light {
    /// Why it casts, a bit each in LIGHT_MOVED's order (span.json); never 0.
    pub moved: u32,
    /// Whether an epoch begins (pxl-engine `light::HEADER_EPOCH`): what a tile holds moved.
    pub epoch: bool,
    /// Whether the box slid or the eye's zooms moved within the epoch, the reads moved with the
    /// box and a tile whose band moved set again (`light_slide_cs`), and whether the world's
    /// changed columns set tiles again within it.
    pub slid: bool,
    pub reached: bool,
    /// The box's corner this cast is cast under, and the one the cast before was
    /// (`light::HEADER_LAST_CORNER`).
    pub corner: [i32; 2],
    pub last_corner: [i32; 2],
    /// The eye's zooms the cast before was cast under (`light::HEADER_LAST_RANGE`).
    pub last_range: [u32; 2],
    /// Whether the world's receivers stand in this cast (`light::HEADER_WORLD_RECEIVERS`); whether
    /// they are added to those that stand, the walked bits holding the claimed tiles' alone,
    /// else stood afresh.
    pub world: bool,
    pub adds: bool,
    /// Whether the receivers' stands take their footprints, the eye standing
    /// (`light::HEADER_FOOTPRINT`).
    pub footprint: bool,
}

/// What a frame runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Plan {
    /// Nothing cast (`plain`), the rays standing (`standing`), the pixels cast again over the
    /// samples that stand (`px`), or a cast run (`cast`).
    pub kind: FrameKind,
    /// Whether every pixel is cast whole though only the claimed tiles' moved, the world's
    /// receivers standing afresh off its walked bits.
    pub ungated: bool,
    /// The light's cast, where it casts.
    pub light: Option<Light>,
}

impl Plan {
    const PLAIN: Plan = Plan {
        kind: FrameKind::plain,
        ungated: false,
        light: None,
    };

    /// Whether the frame casts the pixels' rays.
    pub fn casts(&self) -> bool {
        casts_px(self.kind)
    }
}

/// The state the frames' plans step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Schedule {
    /// Whether the cast's input as last read is what the rays hold: read by the last frame,
    /// which was not dropped, and cast in.
    read: bool,
    /// The jitter the last frame read.
    jitter: [u32; 2],
    /// Whether the eye's walked bits hold every drawn pixel's walk: its last pixels'
    /// cast cast them all.
    walked_whole: bool,
    /// Whether the light's content as last read is what its tiles hold: read by the last
    /// frame, which was not dropped, with the light's cast made.
    content: bool,
    /// The box's corner and the eye's zooms the light last cast under.
    corner: [i32; 2],
    range: [u32; 2],
    /// Whether the next light cast stands the world's receivers on the eye's answers; and
    /// whether the stands of the frame the eye stands in, its pixels' footprints, are owed.
    world: bool,
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

    /// Whether the view and the world stand where the cast's input reads as the last frame's
    /// (`Reads::same`): its rays hold what it reads.
    pub fn stands(&self, same: bool) -> bool {
        same && self.read
    }

    /// The frame's plan from what it read.
    ///
    /// The eye: a cast where the view or the world moved; while they stand, the pixels cast
    /// again off the record where the jitter moved, else nothing.
    ///
    /// The light casts where what a tile holds moved, the box slid, which tiles are read moved,
    /// the footprints are owed or the world moved in columns, while the shadows are on and its
    /// cast made.
    pub fn plan(&mut self, r: Reads) -> Plan {
        let stands = self.stands(r.same);
        self.read = true;
        let jittered = r.jitter != self.jitter;
        self.jitter = r.jitter;
        let mut kind = match (stands, jittered) {
            (true, false) => FrameKind::standing,
            (true, true) => FrameKind::px,
            (false, _) => FrameKind::cast,
        };
        let light_on = r.shadows && r.light_made;
        let held = r.content_same && self.content;
        self.content = r.light_made;
        // A step of the eye's zooms moves what a tile holds only where it moves the tile's band,
        // as a slide does, so it is taken as one (`Light::keeps`).
        let slid = r.corner != self.corner || r.range != self.range;
        // The first frame the eye stands in after it moved marks again, with the footprints
        // the jitter's frames after it stand on.
        let footprint_owed = self.footprint_owed && stands;
        // The world's receivers stand afresh where the light's content or the box moved, and a
        // stand afresh takes every pixel's walk; so do the footprints owed, which the pixels
        // the walked bits leave out owe. Where the walked bits would hold the claimed
        // tiles' alone, as the last pixels' cast or this frame's is gated, every pixel is cast
        // again now, whole (`ungated`), so the bits hold every walk. The one condition the
        // stand follows is then the bits' (`walked_whole`): afresh where they hold every walk;
        // else added to those that stand, the light and the box as they were.
        let partial = if casts_px(kind) {
            r.partial && kind == FrameKind::px
        } else {
            !self.walked_whole
        };
        let ungated = light_on && (!held || slid || footprint_owed) && partial;
        if ungated && kind == FrameKind::standing {
            kind = FrameKind::px;
        }
        let casts = casts_px(kind);
        let walked_whole = !partial || ungated;
        let moved = LightMoved::ALL
            .into_iter()
            .filter(|m| match m {
                LightMoved::content => !held,
                LightMoved::slide => slid,
                LightMoved::receivers => !r.receivers_same,
                LightMoved::footprint => footprint_owed,
                LightMoved::world => r.world_columns,
            })
            .fold(0, |bits, m| bits | 1 << m as u32);
        // The world's receivers stand where the eye's answers lie: stood again by the next
        // cast after the rays were cast anew, where an epoch begins or where the box slid,
        // which moves the tiles they stand on. Else they stand on the tiles they stood on when
        // they last were; the jitter's casts move a ray within its footprint, stood on already
        // where the eye stood when they were.
        self.world |= kind == FrameKind::cast || !held || footprint_owed || slid;
        let light = (light_on && moved != 0).then(|| {
            let last_corner = core::mem::replace(&mut self.corner, r.corner);
            let last_range = core::mem::replace(&mut self.range, r.range);
            // Added to those that stand where the walked bits hold the claimed tiles' alone,
            // which only a cast within the epoch and under the same box takes.
            let adds = !walked_whole;
            xpute_kit::ensure!(!adds || (held && !slid), ENOTRECOVERABLE);
            let world = core::mem::take(&mut self.world);
            // Added, the claimed tiles' alone, the footprint stays owed to the rest.
            if world {
                self.footprint_owed = !stands || adds;
            }
            Light {
                moved,
                epoch: !held,
                slid: held && slid,
                reached: held && r.world_columns,
                corner: r.corner,
                last_corner,
                last_range,
                world,
                adds: world && adds,
                footprint: stands,
            }
        });
        if casts {
            self.walked_whole = !(r.partial && kind == FrameKind::px && !ungated);
        }
        self.plan = Plan {
            kind,
            ungated,
            light,
        };
        self.plan
    }

    /// The frame draws whole after all: its samples' raster drawn whole, and its walked bits
    /// holds every pixel's walk where it casts.
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

    /// No world to cast in: nothing cast, the rays and the map hold nothing read.
    pub fn off(&mut self) {
        (self.read, self.content) = (false, false);
        self.plan = Plan::PLAIN;
    }

    /// The last frame's plan.
    pub fn last(&self) -> &Plan {
        &self.plan
    }

    /// Whether the samples' raster is drawn through its gate, in the claimed tiles alone: the
    /// pixels cast, their walked bits holding those tiles' alone.
    pub fn masked(&self) -> bool {
        self.plan.casts() && !self.walked_whole
    }
}

#[cfg(test)]
#[path = "schedule.test.rs"]
mod test;
