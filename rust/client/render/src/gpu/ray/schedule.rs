// pxl-render/gpu/ray/schedule.rs

//! What each frame runs of the eye's rays and the light's: a pure state machine of what the
//! frame read (`Reads`), stepped once a frame (`Schedule::plan`) and by what befalls the frame
//! after (`whole`, `dropped`, `off`, `light_off`), with no GPU and no clock, so every state it
//! reaches is explored whole on the CPU (schedule.test.rs), joined with the readback of the
//! work each run leaves owed (count.rs `Readback`). The cast (cast.rs `prepare`) reads the
//! frame, asks for a plan and writes what the plan says.

use pxl_engine::count::{EYE, LIGHT};
use pxl_engine::{beam, ray};

use super::casts_px;
use crate::gpu::span_spec::{FrameKind, LightMoved};

/// What a frame read that decides what the eye and the light run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Reads {
    /// Whether the cast's input reads as the last frame's did (cast.rs `prepare`): the eye's
    /// words but the light's, the jitter's and the grid's, the clip levels' table, the shards,
    /// the overwrites' room and revision and the surface, byte for byte; no brick handed fresh;
    /// and the cast's group made.
    pub same: bool,
    /// Where each pixel's ray goes through it (pxl-engine `eye::JITTER`): the world's
    /// sequence, which moves only while a change that is not the scene's animation's settles
    /// (gpu/resolve.rs `asks`). An animation's step holds it, so a turn that only animates
    /// casts no pixel, runs no queue and owes nothing (schedule.test.rs `rests`).
    pub jitter: [u32; 2],
    /// Whether nothing moved but the listed draws that mark (renderer.rs `partial`).
    pub gated: bool,
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
    /// Whether the node cache forgets anything where this frame casts (pxl-engine
    /// `node_cache::FORGETS`).
    pub forgets: bool,
    /// Whether each observer is owed a run of its queue (count.rs `Readback::owed`).
    pub owed: [bool; 2],
    /// Whether the eye's rolled lattice is owed an offset its samples have not all ended at
    /// (count.rs `Readback::lattice`, pxl-engine `count::LATTICE_OWED`).
    pub lattice_owed: bool,
}

/// What the light's cast does, where it casts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Light {
    /// Why it casts, a bit each in LIGHT_MOVED's order (span.json); never 0.
    pub moved: u32,
    /// Whether an epoch begins (pxl-engine `light::HEAD_EPOCH`): what a tile holds moved.
    pub epoch: bool,
    /// Whether the box slid or the eye's zooms moved within the epoch, the reads moved with the
    /// box and a tile whose band moved set again (`light_slide_cs`), and whether the world's
    /// changed columns set tiles again within it.
    pub slid: bool,
    pub reached: bool,
    /// Whether an owed tile's parked rays are resumed where it is claimed
    /// (`light::HEAD_CARRIES`): its cast held the content, the box did not slide, no changed
    /// column reached it, and the node cache forgot nothing since the light last cast.
    pub carries: bool,
    /// The box's corner this cast is cast under, and the one the cast before was
    /// (`light::HEAD_CORNER_WAS`).
    pub corner: [i32; 2],
    pub corner_was: [i32; 2],
    /// The eye's zooms the cast before was cast under (`light::HEAD_RANGE_WAS`).
    pub range_was: [u32; 2],
    /// Whether the world's receivers stand in this cast (`light::HEAD_STANDS_WORLD`); whether
    /// they are added to those that stand, the walked list holding the claimed tiles' alone,
    /// else stood afresh.
    pub world: bool,
    pub adds: bool,
    /// Whether the receivers' stands take their footprints, the eye standing
    /// (`light::HEAD_FOOTPRINT`).
    pub footprint: bool,
}

/// What a frame runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Plan {
    /// Nothing cast (`plain`), the rays standing (`standing`), the pixels cast again over the
    /// samples that stand (`px`), or a cast run (`cast`).
    pub kind: FrameKind,
    /// Whether the grid is laid anew from the eye's words, the rolled lattice begun again at its
    /// first offset (pxl-engine `node_cache::ROLL_K`). Which offset a cast casts its samples
    /// at is the GPU's (roll.wgsl).
    pub lays: bool,
    /// Whether every pixel is cast whole though only the claimed tiles' moved, the world's
    /// receivers standing afresh off its walked list.
    pub ungated: bool,
    /// Whether the cast keeps the states its holes were parked in (`node_cache::RESUMES`);
    /// whether the eye's queue runs alone, casting nothing; and the list its queue takes where
    /// it runs (`ray::holes_from_at`).
    pub resumes: bool,
    pub queue: bool,
    pub from: Option<u32>,
    /// The light's cast, where it casts.
    pub light: Option<Light>,
}

impl Plan {
    const PLAIN: Plan = Plan {
        kind: FrameKind::plain,
        lays: false,
        ungated: false,
        resumes: false,
        queue: false,
        from: None,
        light: None,
    };

    /// Whether the frame casts the pixels' rays.
    pub fn casts(&self) -> bool {
        casts_px(self.kind)
    }

    /// Whether the eye's queue runs: over its cast's walked pixels, or alone.
    pub fn eye_runs(&self) -> bool {
        self.from.is_some()
    }
}

/// The state the frames' plans step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Schedule {
    /// Whether the cast's input as last read is what the rays hold: read by the last frame,
    /// which was not dropped, and cast in.
    read: bool,
    /// The casts of the rolled lattice since the view or the world last moved (`beam::ROLLS`),
    /// and whether the eye's lattice was owed an offset as the last frame read it.
    rolls: u32,
    lattice_owed: bool,
    /// The jitter the last frame read.
    jitter: [u32; 2],
    /// Whether the samples' raster is drawn through its gate, in the claimed tiles alone.
    gated: bool,
    /// Whether the eye's walked pixels' list holds every drawn pixel's walk: its last pixels'
    /// cast cast them all.
    walked_whole: bool,
    /// Whether a pixel the last casts did not take may hold a parked ray of rays no longer
    /// cast: a cast that resumed nothing took the claimed tiles alone. Until a cast that
    /// resumes nothing takes every pixel, no cast resumes.
    parked_stale: bool,
    /// The owed list the eye's queue last filled, which a queue run alone takes
    /// (`ray::owed_into`).
    owed_list: u32,
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
    /// Whether the node cache forgot anything since the light last cast.
    forgot: bool,
    /// Whether the eye may run its queue, a world to cast in; whether the light may cast.
    eye_on: bool,
    light_on: bool,
    /// The last frame's plan.
    plan: Plan,
}

impl Schedule {
    pub const fn new() -> Schedule {
        Schedule {
            read: false,
            rolls: beam::ROLLS,
            lattice_owed: false,
            jitter: [0; 2],
            gated: false,
            walked_whole: false,
            parked_stale: false,
            owed_list: 0,
            content: false,
            corner: [0; 2],
            range: [0; 2],
            world: false,
            footprint_owed: false,
            forgot: false,
            eye_on: false,
            light_on: false,
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
    /// The eye: a cast where the view or the world moved lays the grid and starts the rolled
    /// lattice again; while they stand, the casts that follow keep the grid and cast the
    /// samples at the offset the GPU takes (roll.wgsl), at least `beam::ROLLS` casts and while
    /// the eye's lattice is owed an offset (`Reads::lattice_owed`), whose samples the budget
    /// left parked go on in the next. Then the eye's rays rest: the
    /// frame stands, or, where the jitter moved, casts the pixels again off the record. The
    /// jitter moves where each pixel's ray goes through it, which no sample reads, so it wakes
    /// the pixels and not the roll.
    ///
    /// The light casts where what a tile holds moved, the box slid, which tiles are read moved,
    /// the footprints are owed, the world moved in columns, or its owed work says so, while the
    /// shadows are on and its cast made.
    pub fn plan(&mut self, r: Reads) -> Plan {
        let stands = self.stands(r.same);
        self.read = true;
        let jittered = r.jitter != self.jitter;
        self.jitter = r.jitter;
        self.lattice_owed = r.lattice_owed;
        let rolling = self.rolling();
        let (mut kind, lays) = match (stands, rolling, jittered) {
            (true, false, false) => (FrameKind::standing, false),
            (true, false, true) => (FrameKind::px, false),
            (true, true, _) => (FrameKind::cast, false),
            (false, _, _) => {
                self.rolls = 0;
                (FrameKind::cast, true)
            }
        };
        if kind == FrameKind::cast {
            self.rolls = (self.rolls + 1).min(beam::ROLLS);
        }
        // A cast whose view stood, the roll's alone.
        let rolled = stands && kind == FrameKind::cast;
        let light_on = r.shadows && r.light_made;
        let held = r.content_same && self.content;
        self.content = r.light_made;
        // A step of the eye's zooms moves what a tile holds only where it moves the tile's band,
        // as a slide does, so it is taken as one (`Light::keeps`).
        let slid = r.corner != self.corner || r.range != self.range;
        // The first frame the eye stands in after it moved marks again, with the footprints
        // the jitter's frames after it stand on; and every cast of the roll, whose samples
        // found what the casts before did not.
        let footprint_owed = rolled || (self.footprint_owed && stands);
        // The world's receivers stand afresh where the light's content or the box moved, and a
        // stand afresh takes every pixel's walk; so do the footprints owed, which the pixels
        // the walked list leaves out owe. Where the walked pixels' list would hold the claimed
        // tiles' alone, as the last pixels' cast or this frame's is gated, every pixel is cast
        // again now, whole (`ungated`), so the list holds every walk. The one condition the
        // stand follows is then the list's (`walked_whole`): afresh where it holds every walk;
        // else added to those that stand, the light and the box as they were.
        let partial = if casts_px(kind) {
            r.gated && kind == FrameKind::px
        } else {
            !self.walked_whole
        };
        let ungated = light_on && (!held || slid || footprint_owed) && partial;
        if ungated && kind == FrameKind::standing {
            kind = FrameKind::px;
        }
        let casts = casts_px(kind);
        let walked_whole = !partial || ungated;
        // The eye's rays stand where the view, the jitter, the world and the drawn size are
        // those its last cast walked: a cast then keeps the states its holes were parked in,
        // where every pixel's is of these rays, and a frame that casts nothing runs the eye's
        // queue alone over them where the eye is owed. Any other cast sets every pixel it takes
        // to no parked ray.
        let rays_stand = stands && !jittered;
        let resumes = casts && rays_stand && !self.parked_stale;
        let queue = kind == FrameKind::standing && rays_stand && r.owed[EYE as usize];
        let moved = LightMoved::ALL
            .into_iter()
            .filter(|m| match m {
                LightMoved::content => !held,
                LightMoved::slide => slid,
                LightMoved::receivers => !r.receivers_same,
                LightMoved::footprint => footprint_owed,
                LightMoved::world => r.world_columns,
                LightMoved::owed => r.owed[LIGHT as usize],
            })
            .fold(0, |bits, m| bits | 1 << m as u32);
        self.eye_on = true;
        self.light_on = light_on;
        // The world's receivers stand where the eye's answers lie: stood again by the next
        // cast after the rays were cast anew, where an epoch begins or where the box slid,
        // which moves the tiles they stand on. Else they stand on the tiles they stood on when
        // they last were; the jitter's casts move a ray within its footprint, stood on already
        // where the eye stood when they were.
        self.world |= kind == FrameKind::cast || !held || footprint_owed || slid;
        self.forgot |= casts && r.forgets;
        let light = (light_on && moved != 0).then(|| {
            let corner_was = core::mem::replace(&mut self.corner, r.corner);
            let range_was = core::mem::replace(&mut self.range, r.range);
            // Added to those that stand where the walked list holds the claimed tiles' alone,
            // which only a cast within the epoch and under the same box takes.
            let adds = !walked_whole;
            xpute_kit::ensure!(!adds || (held && !slid), ENOTRECOVERABLE);
            let world = core::mem::take(&mut self.world);
            // Added, the claimed tiles' alone, the footprint stays owed to the rest.
            if world {
                self.footprint_owed = !stands || adds;
            }
            let carries = held && !slid && !r.world_columns && !self.forgot;
            self.forgot = false;
            Light {
                moved,
                epoch: !held,
                slid: held && slid,
                reached: held && r.world_columns,
                carries,
                corner: r.corner,
                corner_was,
                range_was,
                world,
                adds: world && adds,
                footprint: stands,
            }
        });
        // The list the eye's queue takes: its cast's walked pixels, or, run alone, the pixels
        // the queue before left parked; and the owed list it fills in turn.
        let from = (casts || queue).then(|| {
            let from = if casts {
                ray::FROM_WALKED
            } else {
                ray::FROM_OWED + self.owed_list
            };
            self.owed_list = ray::owed_into(from);
            from
        });
        self.gated = r.gated && kind == FrameKind::px && !ungated;
        if casts {
            self.walked_whole = !self.gated;
            if !resumes {
                self.parked_stale = self.gated;
            }
        }
        self.plan = Plan {
            kind,
            lays,
            ungated,
            resumes,
            queue,
            from,
            light,
        };
        self.plan
    }

    /// The frame draws whole after all: its samples' raster drawn whole, and its walked list
    /// holds every pixel's walk where it casts.
    pub fn whole(&mut self) {
        self.gated = false;
        if self.plan.casts() {
            self.walked_whole = true;
            if !self.plan.resumes {
                self.parked_stale = false;
            }
        }
    }

    /// The frame was dropped with its cast: the rays and the map hold older ones, so the next
    /// frame casts both afresh.
    pub fn dropped(&mut self) {
        (self.read, self.content) = (false, false);
    }

    /// No world to cast in: nothing cast, the rays and the map hold nothing read, and the roll
    /// rests, as no view stands.
    pub fn off(&mut self) {
        (self.read, self.content) = (false, false);
        (self.rolls, self.lattice_owed) = (beam::ROLLS, false);
        (self.eye_on, self.light_on) = (false, false);
        self.plan = Plan::PLAIN;
    }

    /// The light's cast let go: the light may not cast until a plan says it may.
    pub fn light_off(&mut self) {
        self.light_on = false;
    }

    /// The last frame's plan.
    pub fn last(&self) -> &Plan {
        &self.plan
    }

    /// Whether the samples' raster is drawn through its gate.
    pub fn gated(&self) -> bool {
        self.gated
    }

    /// Whether the rolled lattice has casts left before the eye's rays rest: fewer than
    /// `beam::ROLLS` cast, or an offset owed.
    pub fn rolling(&self) -> bool {
        self.rolls < beam::ROLLS || self.lattice_owed
    }

    /// The owed list the eye's queue fills, the last run's.
    pub fn owed_list(&self) -> u32 {
        self.owed_list
    }

    /// Whether each observer may run, the eye and the light (count.rs `Readback::wants_frame`).
    pub fn on(&self) -> [bool; 2] {
        [self.eye_on, self.light_on]
    }
}

#[cfg(test)]
#[path = "schedule.test.rs"]
mod test;
