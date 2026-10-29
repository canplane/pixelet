// pxl-kernel/domain/scene/actor.rs

//! The actor kind's behavior: an instance of a registered model that plays
//! its clips. What is its own alone is its clips' times and its joint
//! palette, in a block of its own; where it stands is its holder's, a
//! figure or a case, which hands its stance in (`Stance`). Each tick
//! advances its clips, cross-fades as three's mixer did, and writes the
//! palette into its block.

use pxl_render::frame::draw::{DrawList, PIPELINE_SKINNED};
use pxl_render::gpu::api::word_bytes;
use pxl_spec::space::coord::v3::{self, V3};
use xpute_guest::mem;
use xpute_guest::mem::heap::{alloc, free};

use crate::domain::query::model::registry::{blend, Models};
use crate::domain::scene::frame::Frame;
use crate::domain::view::lens::{cell_there, seen_step_s, Stepped};
use crate::sys::sched::edge::wake_at;
use xpute_guest::clock::now;

#[derive(Clone, Copy)]
struct Playing {
    clip: u32,
    time: f32,
}

/// How its holder stands an actor: where, in units, its turn about Z, and
/// the scale its model is drawn at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stance {
    pub at: V3,
    pub rot_z: f32,
    pub scale: f32,
}

pub struct Actor {
    model: u32,
    cur: Option<Playing>,
    previous: Option<Playing>,
    fade: f32,
    fade_len: f32,
    palette_at: usize,
    palette_bytes: u32,
    palette_handle: u32,
    local: Vec<[f32; 10]>,
    global: Vec<[f32; 16]>,
    /// The fade's source pose, its room kept.
    from: Vec<[f32; 10]>,
    /// Its palette shows its clips at their clocks: posed again only where
    /// it moves where it is seen, or where it does not show them; how many
    /// times it was, and whether it was seen at the last tick.
    posed: bool,
    poses: u32,
    seen: bool,
    /// How long its clip has run while it moves it where it is seen, in
    /// seconds: by the wall clock between its ticks, read at `last`, a stall
    /// taken as one step.
    run: f64,
    last: f64,
    /// Its clip's clock as drawn: `run` stepped a seen step at a time
    /// (`lens::Stepped`), and the reading its clip last took (`shown`).
    clock: Stepped,
    shown: f64,
}

impl Actor {
    /// An instance of `model`, its palette's block taken from the heap; none
    /// where the model is not registered or the heap refuses.
    pub fn new(draw: &mut DrawList, models: &Models, model: u32) -> Option<Actor> {
        let joints = models.get(model)?.joints();
        let palette_bytes = word_bytes(joints.max(1) * 16);
        let (palette_at, palette_bytes) = alloc(palette_bytes)?;
        // SAFETY: the block just acquired; identity joints until the first tick.
        unsafe {
            let p = core::slice::from_raw_parts_mut(
                mem::at::<f32>(palette_at),
                palette_bytes as usize / 4,
            );
            p.fill(0.0);
            for j in 0..joints.max(1) {
                for d in 0..4 {
                    p[j * 16 + d * 5] = 1.0;
                }
            }
        }
        let palette_handle = draw.upload(palette_at as u32, palette_bytes, 0, 0);
        Some(Actor {
            model,
            cur: None,
            previous: None,
            fade: 0.0,
            fade_len: 0.0,
            palette_at,
            palette_bytes,
            palette_handle,
            local: vec![[0.0; 10]; joints],
            global: vec![[0.0; 16]; joints],
            from: Vec::new(),
            posed: false,
            poses: 0,
            seen: false,
            run: 0.0,
            last: 0.0,
            clock: Stepped::default(),
            shown: 0.0,
        })
    }

    /// The handle of its palette's block, which names its draw.
    pub fn palette(&self) -> u32 {
        self.palette_handle
    }

    /// A clip already playing is left alone.
    pub fn play(&mut self, clip: u32, fade_s: f32) {
        if self.cur.is_some_and(|c| c.clip == clip) {
            return;
        }
        self.previous = self.cur;
        self.cur = Some(Playing { clip, time: 0.0 });
        self.posed = false;
        self.fade = 0.0;
        self.fade_len = if self.previous.is_some() {
            fade_s.max(0.0)
        } else {
            0.0
        };
    }

    /// Its block given back.
    pub fn free(self, draw: &mut DrawList) {
        draw.free(self.palette_handle);
        // SAFETY: the block this instance took and has not given back.
        unsafe { free((self.palette_at, self.palette_bytes)) };
    }

    /// Its clock advanced, standing at `stance`; true where the eye can see it
    /// move: in the view and a cell of the camera grid across or more, as
    /// the rays observe nothing smaller, and fading from one clip into the
    /// next. So a frame is asked only for what can be seen to change. A clip
    /// moving it less than a cell holds its time and its pose until the eye
    /// comes close enough to see it move. One moving it a cell or more steps
    /// a seen step of its fastest point at a time and asks no frame: the
    /// turn is woken for the next step and draws it, as the puck's bob is.
    pub fn tick(
        &mut self,
        draw: &mut DrawList,
        models: &Models,
        frame: &Frame,
        stance: &Stance,
    ) -> bool {
        let mut dt = frame.delta_s as f32;
        let Some(m) = models.get(self.model) else {
            return false;
        };
        let length = |p: &Playing| m.duration(p.clip).filter(|&d| d > 0.0);
        let (about, r) = (
            v3::sub(&stance.at, &frame.anchor),
            f64::from(m.reach(self.cur.map(|p| p.clip)) * stance.scale),
        );
        // What spans a camera cell at the actor's distance is observed.
        let distance = v3::len(&v3::sub(&about, &frame.camera));
        let cell_there = cell_there(distance);
        let seen = frame.frustum.sees(about, r) && 2.0 * r >= cell_there;
        self.seen = seen;
        // A clip of some length plays, or one fades into the next.
        let playing = self.cur.filter(|p| length(p).is_some());
        let moving = playing.is_some() || self.previous.is_some();
        let mut moves = seen
            && (self.previous.is_some()
                || playing
                    .is_some_and(|p| f64::from(m.motion(p.clip) * stance.scale) >= cell_there));
        let mut asks = moves;
        // A clip alone, seen to move, steps a seen step at a time; a fade
        // runs a frame a step.
        if let Some(p) = playing.filter(|_| moves && self.previous.is_none()) {
            let step = seen_step_s(distance, f64::from(m.speed(p.clip) * stance.scale));
            let wall = now();
            let since = if self.last > 0.0 {
                (wall - self.last) / 1000.0
            } else {
                frame.delta_s
            };
            self.last = wall;
            self.run += since.clamp(0.0, step.max(frame.delta_s));
            let (time, next, stepped) = self.clock.at(self.run, |at| at + step);
            if next.is_finite() {
                wake_at(wall + (next - self.run) * 1000.0);
            }
            dt = (time - self.shown) as f32;
            self.shown = time;
            (moves, asks) = (stepped, false);
            if stepped {
                crate::domain::scene::region::animated();
            }
        } else {
            self.last = 0.0;
        }
        let wrap = |p: &mut Playing| {
            p.time += dt;
            if let Some(d) = length(p) {
                p.time %= d;
            }
        };
        // Seen, its clip holds where its motion is too small to see.
        let runs = moves || !seen;
        if runs {
            if let Some(p) = self.cur.as_mut() {
                wrap(p);
            }
            if let Some(p) = self.previous.as_mut() {
                wrap(p);
            }
            if self.previous.is_some() {
                self.fade += dt;
                if self.fade >= self.fade_len {
                    self.previous = None;
                }
            }
        }
        let joints = m.joints();
        if joints == 0 {
            return asks;
        }
        // Unseen, its clocks run on and its palette waits for it to be
        // seen; held, its pose stands as it was.
        if !moves && (self.posed || !seen) {
            self.posed &= !(moving && runs);
            return asks;
        }
        self.posed = true;
        self.poses = self.poses.wrapping_add(1);
        m.pose(self.current.map(|p| (p.clip, p.time)), &mut self.local);
        if let Some(prev) = self.previous {
            self.from.resize(joints, [0.0; 10]);
            m.pose(Some((prev.clip, prev.time)), &mut self.from);
            let w = if self.fade_len > 0.0 {
                (self.fade / self.fade_len).clamp(0.0, 1.0)
            } else {
                1.0
            };
            for (to, from) in self.local.iter_mut().zip(self.from.iter()) {
                *to = blend(from, to, w);
            }
        }
        // SAFETY: the actor's own palette block, one mat4 a joint.
        let palette = unsafe {
            core::slice::from_raw_parts_mut(mem::at::<f32>(self.palette_at), joints * 16)
        };
        m.palette(&self.local, &mut self.global, palette);
        draw.rewrite(
            self.palette_handle,
            self.palette_at as u32,
            self.palette_bytes,
        );
        asks
    }

    /// Into the frame's list, standing at `stance`, and where it stands with
    /// it (`DrawList::stand`): every actor drawn is a receiver of the sun's
    /// map, which reads the ones the eye sees.
    pub fn draw(&self, draw: &mut DrawList, models: &Models, stance: &Stance, eye: V3) {
        let Some(m) = models.get(self.model) else {
            return;
        };
        if m.handle() == 0 {
            return;
        }
        // Seen, what it covers and shades as drawn (`region`): as far as
        // its clips, the one it fades from too, reach about where it stands.
        if self.seen {
            let reach = m
                .reach(self.cur.map(|p| p.clip))
                .max(self.previous.map_or(0.0, |p| m.reach(Some(p.clip))));
            let r = f64::from(reach * stance.scale);
            let at = stance.at;
            let state = crate::domain::scene::region::digest(&[
                at[0],
                at[1],
                at[2],
                f64::from(stance.rotation_z),
                f64::from(stance.scale),
                f64::from(self.poses),
            ]);
            crate::domain::scene::region::drawn(self.palette_handle, at, r, true, state);
        }
        let t = DrawList::translation(stance.at, eye);
        draw.draw_scaled(
            PIPELINE_SKINNED,
            m.handle(),
            self.palette_handle,
            self.model,
            t,
            stance.rot_z,
            1.0,
            stance.scale,
        );
        draw.stand(t);
    }
}
